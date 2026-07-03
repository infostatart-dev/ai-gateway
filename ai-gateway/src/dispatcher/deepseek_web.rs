use std::sync::Arc;

use bytes::Bytes;
use deepseek_web::{ExecuteRequest, ExecuteResult, Executor, TurnHook};
use http::{HeaderMap, StatusCode};
use http_body_util::BodyExt;
use serde_json::Value;
use tracing::Instrument;
use url::Url;

use crate::{
    app_state::AppState,
    config::{credentials::ProviderCredentialId, deepseek_web as deepseek_cfg},
    dispatcher::service::{
        Dispatcher,
        outcome::{DispatchOutcome, outcome_from_bytes},
    },
    error::{api::ApiError, internal::InternalError},
    router::{
        budget_aware::DeepSeekWebTrace,
        upstream_failure::{self, UpstreamFailureKind},
    },
    types::{
        extensions::UpstreamFailureContext, provider::InferenceProvider,
        request::Request,
    },
};

struct DeepSeekPreparedRequest {
    user_token: String,
    browser_session: deepseek_web::BrowserSession,
    openai_body: Value,
    stream: bool,
    req_body_bytes: Bytes,
    target_url: Url,
}

impl Dispatcher {
    #[tracing::instrument(
        name = "gateway.web.deepseek.execute",
        skip(self, req)
    )]
    pub(super) async fn dispatch_deepseek_web(
        &self,
        req: Request,
        request_headers: HeaderMap,
        credential_id: Option<&ProviderCredentialId>,
    ) -> Result<DispatchOutcome, ApiError> {
        let prepared = prepare_deepseek_request(
            &self.app_state.config().credentials,
            req,
            credential_id,
        )
        .instrument(tracing::info_span!("gateway.web.deepseek.request_prepare"))
        .await?;
        let turn_hook = turn_pacing_hook(
            self.app_state.clone(),
            self.provider.clone(),
            credential_id.cloned(),
        );
        let executor_span = tracing::info_span!(
            "gateway.web.deepseek.executor",
            turns = tracing::field::Empty,
            upload_parts = tracing::field::Empty,
            pow_cache_hits = tracing::field::Empty,
        );

        match Executor::default()
            .execute(ExecuteRequest {
                user_token: prepared.user_token,
                browser_session: Some(prepared.browser_session),
                body: prepared.openai_body,
                stream: prepared.stream,
                turn_hook: Some(turn_hook),
            })
            .instrument(executor_span.clone())
            .await
        {
            Ok(result) => {
                executor_span.record("turns", result.stats.turns);
                executor_span.record("upload_parts", result.stats.upload_parts);
                executor_span
                    .record("pow_cache_hits", result.stats.pow_cache_hits);
                tracing::info_span!("gateway.web.deepseek.request_finalize")
                    .in_scope(|| {
                        build_deepseek_success_outcome(
                            result,
                            prepared.stream,
                            prepared.target_url,
                            prepared.req_body_bytes,
                            request_headers,
                        )
                    })
            }
            Err(error) => {
                tracing::warn!(error = %error, "deepseek-web executor failed");
                tracing::info_span!("gateway.web.deepseek.request_finalize")
                    .in_scope(|| {
                        build_deepseek_error_outcome(
                            error,
                            prepared.target_url,
                            prepared.req_body_bytes,
                            request_headers,
                        )
                    })
            }
        }
    }
}

async fn prepare_deepseek_request(
    registry: &crate::config::credentials::CredentialRegistry,
    req: Request,
    credential_id: Option<&ProviderCredentialId>,
) -> Result<DeepSeekPreparedRequest, ApiError> {
    let session_path = resolve_session_path(
        registry,
        credential_id,
        deepseek_cfg::DEFAULT_CREDENTIAL_ID,
    )
    .ok_or_else(|| ApiError::Internal(InternalError::ProviderNotFound))?;
    let browser_session = deepseek_cfg::load_browser_session(&session_path)
        .ok_or_else(|| ApiError::Internal(InternalError::ProviderNotFound))?;
    let user_token = browser_session.token.clone();

    let body_bytes = req
        .into_body()
        .collect()
        .await
        .map_err(|e| InternalError::RequestBodyError(Box::new(e)))?
        .to_bytes();
    let openai_body: Value =
        serde_json::from_slice(&body_bytes).map_err(|e| {
            ApiError::Internal(InternalError::Deserialize {
                ty: "chat completion request",
                error: e,
            })
        })?;
    let stream = openai_body
        .get("stream")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let target_url = Url::parse(deepseek_web::COMPLETION_URL)
        .map_err(|_| InternalError::Internal)?;

    Ok(DeepSeekPreparedRequest {
        user_token,
        browser_session,
        openai_body,
        stream,
        req_body_bytes: body_bytes,
        target_url,
    })
}

fn build_deepseek_error_outcome(
    error: deepseek_web::Error,
    target_url: Url,
    req_body_bytes: Bytes,
    request_headers: HeaderMap,
) -> Result<DispatchOutcome, ApiError> {
    let (status, body, failure_ctx) = deepseek_error_response(error)?;
    let mut outcome = outcome_from_bytes(
        status,
        HeaderMap::new(),
        &body,
        target_url,
        req_body_bytes,
        request_headers,
    )
    .map_err(ApiError::Internal)?;
    if let Some(ctx) = failure_ctx {
        outcome.response.extensions_mut().insert(ctx);
    }
    Ok(outcome)
}

fn build_deepseek_success_outcome(
    result: ExecuteResult,
    stream: bool,
    target_url: Url,
    req_body_bytes: Bytes,
    request_headers: HeaderMap,
) -> Result<DispatchOutcome, ApiError> {
    let status =
        StatusCode::from_u16(result.status).unwrap_or(StatusCode::BAD_GATEWAY);
    let mut headers = HeaderMap::new();
    let content_type = if stream {
        "text/event-stream"
    } else {
        "application/json"
    };
    headers.insert(
        http::header::CONTENT_TYPE,
        http::HeaderValue::from_static(content_type),
    );

    let mut outcome = outcome_from_bytes(
        status,
        headers,
        &Bytes::from(result.body),
        target_url,
        req_body_bytes,
        request_headers,
    )
    .map_err(ApiError::Internal)?;
    outcome.response.extensions_mut().insert(DeepSeekWebTrace {
        turns: result.stats.turns,
        upload_parts: result.stats.upload_parts,
        pow_cache_hits: result.stats.pow_cache_hits,
    });
    Ok(outcome)
}

fn turn_pacing_hook(
    app_state: AppState,
    provider: InferenceProvider,
    credential_id: Option<ProviderCredentialId>,
) -> TurnHook {
    Arc::new(move || {
        let app_state = app_state.clone();
        let provider = provider.clone();
        let credential_id = credential_id.clone();
        Box::pin(async move {
            crate::router::pacing::acquire_upstream_pacing(
                &app_state,
                &provider,
                credential_id.as_ref(),
                credential_id.as_ref().and_then(|id| {
                    app_state
                        .config()
                        .credentials
                        .get(id)
                        .map(|c| c.tier.as_str())
                }),
                None,
                0,
            )
            .await
            .map_err(|e| deepseek_web::Error::Other(e.to_string()))?;
            Ok(())
        })
    })
}

fn resolve_session_path(
    registry: &crate::config::credentials::CredentialRegistry,
    credential_id: Option<&ProviderCredentialId>,
    default_id: &str,
) -> Option<std::path::PathBuf> {
    if let Some(id) = credential_id
        && let Some(cred) = registry.get(id)
        && let Some(path) = cred.key.as_secret()
    {
        return Some(std::path::PathBuf::from(path.expose()));
    }
    deepseek_cfg::session_path_for_credential(default_id)
}

fn deepseek_error_response(
    error: deepseek_web::Error,
) -> Result<(StatusCode, Bytes, Option<UpstreamFailureContext>), InternalError>
{
    match error {
        deepseek_web::Error::CredentialRestricted {
            message,
            restricted_until,
        } => {
            let until = restricted_until
                .map(upstream_failure::unix_secs_to_utc)
                .map(|dt| dt.to_rfc3339());
            let mut error_json = serde_json::json!({
                "error": {
                    "message": message,
                    "type": "invalid_request_error",
                    "param": null,
                    "code": "credential_restricted",
                }
            });
            if let Some(until) = &until {
                error_json["error"]["restricted_until"] =
                    serde_json::Value::String(until.clone());
            }
            let restricted_until_dt =
                restricted_until.map(upstream_failure::unix_secs_to_utc);
            Ok((
                upstream_failure::credential_restricted_http_status(),
                Bytes::from(error_json.to_string()),
                Some(UpstreamFailureContext {
                    kind: UpstreamFailureKind::CredentialRestricted,
                    restricted_until: restricted_until_dt,
                }),
            ))
        }
        deepseek_web::Error::SessionAuth(message) => {
            let (status, body) = simple_error_body(
                StatusCode::UNAUTHORIZED,
                &message,
                "authentication_error",
                Some("invalid_session"),
            );
            Ok((status, body, None))
        }
        deepseek_web::Error::EmptyResponse => Ok(simple_error_outcome(
            StatusCode::BAD_GATEWAY,
            "empty response from DeepSeek",
            "server_error",
            None,
        )),
        deepseek_web::Error::Other(message) => Ok(simple_error_outcome(
            StatusCode::BAD_GATEWAY,
            &message,
            "server_error",
            None,
        )),
        deepseek_web::Error::Upstream { status, message } => {
            Ok(simple_error_outcome(
                StatusCode::from_u16(status).unwrap_or(StatusCode::BAD_GATEWAY),
                &message,
                "server_error",
                None,
            ))
        }
        _ => Err(InternalError::Internal),
    }
}

fn simple_error_outcome(
    status: StatusCode,
    message: &str,
    error_type: &str,
    code: Option<&str>,
) -> (StatusCode, Bytes, Option<UpstreamFailureContext>) {
    let (status, body) = simple_error_body(status, message, error_type, code);
    (status, body, None)
}

fn simple_error_body(
    status: StatusCode,
    message: &str,
    error_type: &str,
    code: Option<&str>,
) -> (StatusCode, Bytes) {
    let body = serde_json::json!({
        "error": {
            "message": message,
            "type": error_type,
            "param": null,
            "code": code,
        }
    });
    (status, Bytes::from(body.to_string()))
}

#[cfg(test)]
mod tests {
    use deepseek_web::Error;

    use super::deepseek_error_response;

    #[test]
    fn session_auth_maps_to_401() {
        let (status, body, ctx) =
            deepseek_error_response(Error::SessionAuth("expired".into()))
                .unwrap();
        assert_eq!(status, http::StatusCode::UNAUTHORIZED);
        assert!(ctx.is_none());
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["error"]["code"], "invalid_session");
    }

    #[test]
    fn credential_restricted_maps_to_403_with_extension() {
        let (status, body, ctx) =
            deepseek_error_response(Error::CredentialRestricted {
                message: "user is muted".into(),
                restricted_until: Some(1_781_861_651),
            })
            .unwrap();
        assert_eq!(status, http::StatusCode::FORBIDDEN);
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["error"]["code"], "credential_restricted");
        assert!(json["error"]["restricted_until"].is_string());
        assert!(ctx.is_some());
    }

    #[test]
    fn credential_restricted_is_not_empty_response_502() {
        let (status, body, ctx) =
            deepseek_error_response(Error::CredentialRestricted {
                message: "user is muted".into(),
                restricted_until: None,
            })
            .unwrap();
        assert_ne!(status, http::StatusCode::BAD_GATEWAY);
        assert!(ctx.is_some());
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["error"]["code"], "credential_restricted");
    }

    #[test]
    fn upstream_error_preserves_status() {
        let (status, _, ctx) = deepseek_error_response(Error::Upstream {
            status: 429,
            message: "rate limited".into(),
        })
        .unwrap();
        assert_eq!(status, http::StatusCode::TOO_MANY_REQUESTS);
        assert!(ctx.is_none());
    }
}
