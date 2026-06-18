use ai_gateway::{
    config::providers::ProvidersConfig, types::provider::InferenceProvider,
};
use axum::{
    http::{HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
};
use serde_json::{Value, json};

use crate::{
    capability::{ContentResult, resolve_content},
    family::ProtocolFamily,
    limits::RateLimitVerdict,
    payload::{anthropic_message, openai_chat_completion, openai_sse_chunks},
    tokens::estimate_usage,
    welcome::WELCOME,
};

pub fn render_api_family(
    family: ProtocolFamily,
    body: &Value,
    providers: &ProvidersConfig,
    provider: &InferenceProvider,
) -> Response {
    let content = match resolve_content(body, providers, provider) {
        ContentResult::Ok(s) => s,
        ContentResult::UnsupportedFormat => {
            return unsupported_format_response(family);
        }
    };
    let stream = body.get("stream").and_then(Value::as_bool) == Some(true);
    let usage = estimate_usage(body, &content);
    let json = match family {
        ProtocolFamily::AnthropicMessages => anthropic_message(&content, usage),
        ProtocolFamily::GeminiOpenAiCompat | ProtocolFamily::OpenAiCompat => {
            openai_chat_completion(&content, usage)
        }
    };
    if stream {
        let stream_content = json
            .pointer("/choices/0/message/content")
            .or_else(|| json.pointer("/content/0/text"))
            .and_then(Value::as_str)
            .unwrap_or(WELCOME);
        return sse(&openai_sse_chunks(stream_content, usage));
    }
    json_ok(json)
}

pub fn rate_limit_response(
    family: ProtocolFamily,
    verdict: RateLimitVerdict,
) -> Response {
    if verdict == RateLimitVerdict::RpdExceeded {
        return quota_exhausted_response();
    }
    if verdict == RateLimitVerdict::Concurrent {
        return overload_response();
    }
    (
        StatusCode::TOO_MANY_REQUESTS,
        [(header::RETRY_AFTER, HeaderValue::from_static("1"))],
        axum::Json(rate_limit_json(family)),
    )
        .into_response()
}

pub fn auth_error_response() -> Response {
    (
        StatusCode::UNAUTHORIZED,
        axum::Json(json!({ "error": { "message": "invalid api key" } })),
    )
        .into_response()
}

pub fn credential_restricted_response() -> Response {
    (
        StatusCode::FORBIDDEN,
        axum::Json(json!({
            "error": {
                "message": "user is muted",
                "code": "credential_restricted",
                "restricted_until": "2026-06-19T09:34:11Z"
            }
        })),
    )
        .into_response()
}

pub fn quota_exhausted_response() -> Response {
    (
        StatusCode::TOO_MANY_REQUESTS,
        axum::Json(
            json!({ "error": { "message": "You exceeded your daily limit." } }),
        ),
    )
        .into_response()
}

pub fn overload_response() -> Response {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        axum::Json(json!({ "error": { "message": "model is overloaded" } })),
    )
        .into_response()
}

pub fn not_found_response() -> Response {
    (
        StatusCode::NOT_FOUND,
        axum::Json(json!({
            "error": {
                "message": "models/gemini-3.5-flash-preview is not found for API version v1beta"
            }
        })),
    )
        .into_response()
}

pub fn high_demand_response() -> Response {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        axum::Json(json!({
            "error": {
                "message": "This model is currently experiencing high demand. Please try again later."
            }
        })),
    )
        .into_response()
}

pub fn never_purchased_response() -> Response {
    (
        StatusCode::PAYMENT_REQUIRED,
        axum::Json(json!({
            "error": {
                "message": "You have never purchased credits. Only free models are available."
            }
        })),
    )
        .into_response()
}

pub fn free_models_per_day_response() -> Response {
    let reset_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
        .saturating_add(120_000);
    (
        StatusCode::TOO_MANY_REQUESTS,
        [(
            "x-ratelimit-reset",
            HeaderValue::from_str(&reset_ms.to_string())
                .unwrap_or_else(|_| HeaderValue::from_static("0")),
        )],
        axum::Json(json!({
            "error": {
                "message": "Rate limit exceeded: free-models-per-day"
            }
        })),
    )
        .into_response()
}

fn rate_limit_json(family: ProtocolFamily) -> Value {
    let message = match family {
        ProtocolFamily::GeminiOpenAiCompat => {
            "Resource has been exhausted (e.g. check quota)."
        }
        ProtocolFamily::OpenAiCompat | ProtocolFamily::AnthropicMessages => {
            "Rate limit reached for organization. Please try again in 1s."
        }
    };
    json!({ "error": { "message": message, "type": "rate_limit_error" } })
}

fn unsupported_format_response(family: ProtocolFamily) -> Response {
    let message = match family {
        ProtocolFamily::AnthropicMessages => {
            "structured output (json_schema) is not supported for this model"
        }
        ProtocolFamily::GeminiOpenAiCompat | ProtocolFamily::OpenAiCompat => {
            "This model does not support response_format type 'json_schema'."
        }
    };
    (
        StatusCode::UNPROCESSABLE_ENTITY,
        axum::Json(json!({ "error": { "message": message, "type": "invalid_request_error" } })),
    )
        .into_response()
}

fn json_ok(value: Value) -> Response {
    (StatusCode::OK, axum::Json(value)).into_response()
}

fn sse(body: &str) -> Response {
    (
        StatusCode::OK,
        [(
            header::CONTENT_TYPE,
            HeaderValue::from_static("text/event-stream"),
        )],
        body.to_string(),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use ai_gateway::{
        config::providers::ProvidersConfig, types::provider::InferenceProvider,
    };
    use serde_json::json;
    use web_structured_output::{
        check_structured_response, parse_json_schema_spec,
    };

    use super::*;

    #[test]
    fn high_demand_and_not_found_templates_match_google_shapes() {
        let not_found = not_found_response();
        assert_eq!(not_found.status(), StatusCode::NOT_FOUND);
        let high_demand = high_demand_response();
        assert_eq!(high_demand.status(), StatusCode::SERVICE_UNAVAILABLE);
    }

    #[test]
    fn openrouter_wire_profiles_match_live_shapes() {
        let unpaid = never_purchased_response();
        assert_eq!(unpaid.status(), StatusCode::PAYMENT_REQUIRED);
        let daily = free_models_per_day_response();
        assert_eq!(daily.status(), StatusCode::TOO_MANY_REQUESTS);
        assert!(daily.headers().contains_key("x-ratelimit-reset"));
    }

    #[tokio::test]
    async fn credential_restricted_profile_matches_openai_error_shape() {
        let response = credential_restricted_response();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["error"]["code"], "credential_restricted");
    }

    #[test]
    fn capable_model_returns_200_with_valid_schema() {
        let providers = ProvidersConfig::default();
        let provider = InferenceProvider::Named("longcat".into());
        let request = json!({
            "model": "LongCat-Flash-Lite",
            "response_format": {
                "type": "json_schema",
                "json_schema": {
                    "strict": true,
                    "schema": {
                        "type": "object",
                        "properties": { "status": { "type": "string" } },
                        "required": ["status"],
                        "additionalProperties": false
                    }
                }
            }
        });
        let response = render_api_family(
            ProtocolFamily::OpenAiCompat,
            &request,
            &providers,
            &provider,
        );
        assert_eq!(response.status(), StatusCode::OK);
        let spec = parse_json_schema_spec(&request).unwrap();
        // Verify content matches schema by checking capability directly
        let content = crate::capability::assistant_content(
            &request, &providers, &provider,
        );
        let assistant =
            json!({ "choices": [{ "message": { "content": content } }] });
        assert!(check_structured_response(&assistant, Some(&spec)).is_none());
    }

    #[test]
    fn non_capable_model_returns_422_for_json_schema_request() {
        let providers = ProvidersConfig::default();
        // longcat's LongCat-Flash-Thinking doesn't support json_schema
        let provider = InferenceProvider::Named("longcat".into());
        let request = json!({
            "model": "LongCat-Flash-Thinking",
            "response_format": {
                "type": "json_schema",
                "json_schema": {
                    "strict": true,
                    "schema": {
                        "type": "object",
                        "properties": { "answer": { "type": "string" } },
                        "required": ["answer"],
                        "additionalProperties": false
                    }
                }
            }
        });
        let response = render_api_family(
            ProtocolFamily::OpenAiCompat,
            &request,
            &providers,
            &provider,
        );
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    }

    #[test]
    fn fat_payload_usage_is_not_stubbed() {
        let filler = "x".repeat(12_000);
        let body = serde_json::json!({
            "model": "default",
            "messages": [{"role": "user", "content": filler}]
        });
        let usage =
            estimate_usage(&body, r#"{"value":"ok","details":"routing load"}"#);
        assert!(
            usage.prompt_tokens > 1000,
            "expected fat prompt tokens, got {}",
            usage.prompt_tokens
        );
    }
}
