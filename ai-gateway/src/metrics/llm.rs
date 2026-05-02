use bytes::Bytes;
use http::{HeaderMap, HeaderValue, StatusCode};
use opentelemetry::{
    KeyValue,
    metrics::{Counter, Gauge, Histogram, Meter},
};
use serde_json::Value;

use crate::types::{
    extensions::RequestKind, model_id::ModelId, provider::InferenceProvider,
    router::RouterId,
};

#[derive(Debug, Clone)]
pub struct LlmMetrics {
    pub provider_requests: Counter<u64>,
    pub provider_request_body_bytes: Counter<u64>,
    pub provider_response_body_bytes: Counter<u64>,
    pub provider_tokens: Counter<u64>,
    pub provider_response_duration: Histogram<f64>,
    pub provider_rate_limit_limit: Gauge<u64>,
    pub provider_rate_limit_remaining: Gauge<u64>,
    pub provider_rate_limit_reset: Gauge<f64>,
}

impl LlmMetrics {
    #[must_use]
    pub fn new(meter: &Meter) -> Self {
        let provider_requests = meter
            .u64_counter("llm_provider_requests")
            .with_description("Number of upstream LLM provider responses")
            .build();
        let provider_request_body_bytes = meter
            .u64_counter("llm_provider_request_body_bytes")
            .with_description(
                "Request body bytes sent to upstream LLM providers",
            )
            .build();
        let provider_response_body_bytes = meter
            .u64_counter("llm_provider_response_body_bytes")
            .with_description(
                "Response body bytes returned by upstream LLM providers",
            )
            .build();
        let provider_tokens = meter
            .u64_counter("llm_provider_tokens")
            .with_description("Tokens reported by upstream LLM providers")
            .build();
        let provider_response_duration = meter
            .f64_histogram("llm_provider_response_duration")
            .with_unit("ms")
            .with_description(
                "Duration until an upstream LLM provider response body \
                 completes",
            )
            .build();
        let provider_rate_limit_limit = meter
            .u64_gauge("llm_provider_rate_limit_limit")
            .with_description("Provider rate-limit limit from response headers")
            .build();
        let provider_rate_limit_remaining = meter
            .u64_gauge("llm_provider_rate_limit_remaining")
            .with_description(
                "Provider rate-limit remaining value from response headers",
            )
            .build();
        let provider_rate_limit_reset = meter
            .f64_gauge("llm_provider_rate_limit_reset_seconds")
            .with_unit("s")
            .with_description(
                "Provider rate-limit reset duration from response headers",
            )
            .build();
        Self {
            provider_requests,
            provider_request_body_bytes,
            provider_response_body_bytes,
            provider_tokens,
            provider_response_duration,
            provider_rate_limit_limit,
            provider_rate_limit_remaining,
            provider_rate_limit_reset,
        }
    }

    pub fn record_provider_tokens(
        &self,
        usage: TokenUsage,
        attrs: &[KeyValue],
    ) {
        for (token_type, value) in usage.reported_values() {
            let mut attrs = attrs.to_vec();
            attrs.push(KeyValue::new("token_type", token_type));
            self.provider_tokens.add(value, &attrs);
        }
    }

    pub fn record_rate_limit_headers(
        &self,
        headers: &HeaderMap,
        attrs: &[KeyValue],
    ) {
        self.record_u64_header(
            headers,
            "x-ratelimit-limit-requests",
            "requests",
            attrs,
            RateLimitMetric::Limit,
        );
        self.record_u64_header(
            headers,
            "x-ratelimit-limit-tokens",
            "tokens",
            attrs,
            RateLimitMetric::Limit,
        );
        self.record_u64_header(
            headers,
            "x-ratelimit-remaining-requests",
            "requests",
            attrs,
            RateLimitMetric::Remaining,
        );
        self.record_u64_header(
            headers,
            "x-ratelimit-remaining-tokens",
            "tokens",
            attrs,
            RateLimitMetric::Remaining,
        );
        self.record_reset_header(
            headers,
            "x-ratelimit-reset-requests",
            "requests",
            attrs,
        );
        self.record_reset_header(
            headers,
            "x-ratelimit-reset-tokens",
            "tokens",
            attrs,
        );
    }

    fn record_u64_header(
        &self,
        headers: &HeaderMap,
        name: &'static str,
        quota: &'static str,
        attrs: &[KeyValue],
        metric: RateLimitMetric,
    ) {
        let Some(value) = header_u64(headers.get(name)) else {
            return;
        };
        let mut attrs = attrs.to_vec();
        attrs.push(KeyValue::new("quota", quota));
        match metric {
            RateLimitMetric::Limit => {
                self.provider_rate_limit_limit.record(value, &attrs);
            }
            RateLimitMetric::Remaining => {
                self.provider_rate_limit_remaining.record(value, &attrs);
            }
        }
    }

    fn record_reset_header(
        &self,
        headers: &HeaderMap,
        name: &'static str,
        quota: &'static str,
        attrs: &[KeyValue],
    ) {
        let Some(value) = headers
            .get(name)
            .and_then(|value| value.to_str().ok())
            .and_then(parse_duration_secs)
        else {
            return;
        };
        let mut attrs = attrs.to_vec();
        attrs.push(KeyValue::new("quota", quota));
        self.provider_rate_limit_reset.record(value, &attrs);
    }
}

#[derive(Clone, Copy)]
enum RateLimitMetric {
    Limit,
    Remaining,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TokenUsage {
    pub input: Option<u64>,
    pub output: Option<u64>,
    pub total: Option<u64>,
    pub cached: Option<u64>,
    pub reasoning: Option<u64>,
}

impl TokenUsage {
    #[must_use]
    pub fn is_empty(self) -> bool {
        self.input.is_none()
            && self.output.is_none()
            && self.total.is_none()
            && self.cached.is_none()
            && self.reasoning.is_none()
    }

    fn merge_max(&mut self, other: Self) {
        self.input = max_option(self.input, other.input);
        self.output = max_option(self.output, other.output);
        self.total = max_option(self.total, other.total);
        self.cached = max_option(self.cached, other.cached);
        self.reasoning = max_option(self.reasoning, other.reasoning);
    }

    fn reported_values(self) -> Vec<(&'static str, u64)> {
        let mut values = Vec::new();
        if let Some(value) = self.input {
            values.push(("input", value));
        }
        if let Some(value) = self.output {
            values.push(("output", value));
        }
        if let Some(value) = self
            .total
            .or_else(|| Some(self.input?.saturating_add(self.output?)))
        {
            values.push(("total", value));
        }
        if let Some(value) = self.cached {
            values.push(("cached", value));
        }
        if let Some(value) = self.reasoning {
            values.push(("reasoning", value));
        }
        values
    }
}

#[must_use]
pub fn provider_attrs(
    provider: &InferenceProvider,
    model: Option<&ModelId>,
    router_id: Option<&RouterId>,
    path: &str,
    status: StatusCode,
    is_stream: bool,
    request_kind: RequestKind,
) -> Vec<KeyValue> {
    vec![
        KeyValue::new("provider", provider.to_string()),
        KeyValue::new(
            "model",
            model.map_or_else(|| "unknown".to_string(), ToString::to_string),
        ),
        KeyValue::new(
            "router_id",
            router_id
                .map_or_else(|| "unknown".to_string(), ToString::to_string),
        ),
        KeyValue::new("provider_path", path.to_string()),
        KeyValue::new("status_code", i64::from(status.as_u16())),
        KeyValue::new("status_class", status_class(status)),
        KeyValue::new("stream", is_stream),
        KeyValue::new("request_kind", request_kind_name(request_kind)),
    ]
}

#[must_use]
pub fn extract_usage_from_response_body(
    body: &Bytes,
    is_stream: bool,
) -> TokenUsage {
    if is_stream {
        extract_stream_usage(body)
    } else {
        serde_json::from_slice::<Value>(body)
            .ok()
            .map_or_else(TokenUsage::default, |value| {
                extract_usage_from_json(&value)
            })
    }
}

fn extract_stream_usage(body: &Bytes) -> TokenUsage {
    let Ok(text) = std::str::from_utf8(body) else {
        return TokenUsage::default();
    };

    let mut usage = TokenUsage::default();
    let mut parsed_event = false;

    for line in text.lines() {
        let line = line.trim();
        let Some(data) = line.strip_prefix("data:") else {
            continue;
        };
        let data = data.trim();
        if data.is_empty() || data == "[DONE]" {
            continue;
        }
        parsed_event = true;
        if let Ok(value) = serde_json::from_str::<Value>(data) {
            usage.merge_max(extract_usage_from_json(&value));
        }
    }

    if parsed_event {
        usage
    } else {
        serde_json::from_slice::<Value>(body)
            .ok()
            .map_or_else(TokenUsage::default, |value| {
                extract_usage_from_json(&value)
            })
    }
}

fn extract_usage_from_json(value: &Value) -> TokenUsage {
    if let Some(usage) = value.get("usage") {
        usage_from_value(usage)
    } else if let Some(usage) = value.get("usageMetadata") {
        gemini_usage_from_value(usage)
    } else {
        usage_from_value(value)
    }
}

fn usage_from_value(value: &Value) -> TokenUsage {
    TokenUsage {
        input: first_u64(
            value,
            &["prompt_tokens", "input_tokens", "inputTokens"],
        ),
        output: first_u64(
            value,
            &["completion_tokens", "output_tokens", "outputTokens"],
        ),
        total: first_u64(value, &["total_tokens", "totalTokens"]),
        cached: first_nested_u64(
            value,
            &[
                &["prompt_tokens_details", "cached_tokens"],
                &["input_token_details", "cached_tokens"],
                &["input_token_details", "cache_read"],
                &["cache_read_input_tokens"],
                &["cache_creation_input_tokens"],
            ],
        ),
        reasoning: first_nested_u64(
            value,
            &[
                &["completion_tokens_details", "reasoning_tokens"],
                &["output_token_details", "reasoning_tokens"],
                &["reasoning_tokens"],
            ],
        ),
    }
}

fn gemini_usage_from_value(value: &Value) -> TokenUsage {
    TokenUsage {
        input: first_u64(value, &["promptTokenCount"]),
        output: first_u64(value, &["candidatesTokenCount"]),
        total: first_u64(value, &["totalTokenCount"]),
        cached: first_u64(value, &["cachedContentTokenCount"]),
        reasoning: first_u64(value, &["thoughtsTokenCount"]),
    }
}

fn first_u64(value: &Value, keys: &[&str]) -> Option<u64> {
    keys.iter().find_map(|key| value.get(*key)?.as_u64())
}

fn first_nested_u64(value: &Value, paths: &[&[&str]]) -> Option<u64> {
    paths.iter().find_map(|path| {
        let mut current = value;
        for key in *path {
            current = current.get(*key)?;
        }
        current.as_u64()
    })
}

fn header_u64(value: Option<&HeaderValue>) -> Option<u64> {
    header_str(value)?.trim().parse().ok()
}

fn header_str(value: Option<&HeaderValue>) -> Option<&str> {
    value?.to_str().ok()
}

fn parse_duration_secs(value: &str) -> Option<f64> {
    if let Ok(seconds) = value.trim().parse::<f64>() {
        return Some(seconds);
    }

    let mut total = 0.0;
    let mut rest = value.trim();
    while !rest.is_empty() {
        let end = rest
            .find(|c: char| !(c.is_ascii_digit() || c == '.'))
            .unwrap_or(rest.len());
        if end == 0 {
            return None;
        }
        let number = rest[..end].parse::<f64>().ok()?;
        rest = &rest[end..];

        if let Some(next) = rest.strip_prefix("ms") {
            total += number / 1000.0;
            rest = next;
        } else if let Some(next) = rest.strip_prefix('s') {
            total += number;
            rest = next;
        } else if let Some(next) = rest.strip_prefix('m') {
            total += number * 60.0;
            rest = next;
        } else if let Some(next) = rest.strip_prefix('h') {
            total += number * 3600.0;
            rest = next;
        } else if let Some(next) = rest.strip_prefix('d') {
            total += number * 86_400.0;
            rest = next;
        } else {
            return None;
        }
    }
    Some(total)
}

fn max_option(left: Option<u64>, right: Option<u64>) -> Option<u64> {
    match (left, right) {
        (Some(left), Some(right)) => Some(left.max(right)),
        (Some(value), None) | (None, Some(value)) => Some(value),
        (None, None) => None,
    }
}

fn status_class(status: StatusCode) -> &'static str {
    if status.is_informational() {
        "1xx"
    } else if status.is_success() {
        "2xx"
    } else if status.is_redirection() {
        "3xx"
    } else if status.is_client_error() {
        "4xx"
    } else if status.is_server_error() {
        "5xx"
    } else {
        "unknown"
    }
}

fn request_kind_name(request_kind: RequestKind) -> &'static str {
    match request_kind {
        RequestKind::Router => "router",
        RequestKind::UnifiedApi => "unified_api",
        RequestKind::DirectProxy => "direct_proxy",
    }
}

#[cfg(test)]
mod tests {
    use bytes::Bytes;
    use serde_json::json;

    use super::*;

    #[test]
    fn extracts_openai_usage() {
        let body = Bytes::from(
            json!({
                "usage": {
                    "prompt_tokens": 10,
                    "completion_tokens": 7,
                    "total_tokens": 17,
                    "prompt_tokens_details": {
                        "cached_tokens": 3
                    },
                    "completion_tokens_details": {
                        "reasoning_tokens": 2
                    }
                }
            })
            .to_string(),
        );

        assert_eq!(
            extract_usage_from_response_body(&body, false),
            TokenUsage {
                input: Some(10),
                output: Some(7),
                total: Some(17),
                cached: Some(3),
                reasoning: Some(2),
            }
        );
    }

    #[test]
    fn extracts_anthropic_usage() {
        let body = Bytes::from(
            json!({
                "usage": {
                    "input_tokens": 11,
                    "output_tokens": 13,
                    "cache_read_input_tokens": 5
                }
            })
            .to_string(),
        );

        assert_eq!(
            extract_usage_from_response_body(&body, false),
            TokenUsage {
                input: Some(11),
                output: Some(13),
                total: None,
                cached: Some(5),
                reasoning: None,
            }
        );
    }

    #[test]
    fn extracts_gemini_usage_metadata() {
        let body = Bytes::from(
            json!({
                "usageMetadata": {
                    "promptTokenCount": 9,
                    "candidatesTokenCount": 4,
                    "totalTokenCount": 13,
                    "cachedContentTokenCount": 2,
                    "thoughtsTokenCount": 1
                }
            })
            .to_string(),
        );

        assert_eq!(
            extract_usage_from_response_body(&body, false),
            TokenUsage {
                input: Some(9),
                output: Some(4),
                total: Some(13),
                cached: Some(2),
                reasoning: Some(1),
            }
        );
    }

    #[test]
    fn extracts_streamed_usage() {
        let body = Bytes::from(
            r#"data: {"choices":[],"usage":{"prompt_tokens":10,"completion_tokens":5,"total_tokens":15}}

data: [DONE]

"#,
        );

        assert_eq!(
            extract_usage_from_response_body(&body, true),
            TokenUsage {
                input: Some(10),
                output: Some(5),
                total: Some(15),
                cached: None,
                reasoning: None,
            }
        );
    }

    #[test]
    fn parses_provider_reset_header_durations() {
        assert_eq!(parse_duration_secs("1s"), Some(1.0));
        assert_eq!(parse_duration_secs("6m0s"), Some(360.0));
        assert_eq!(parse_duration_secs("250ms"), Some(0.25));
    }
}
