use serde_json::Value;

use super::{
    classify::FailureKind,
    duration::{parse_retry_delay_seconds, parse_try_again_in_seconds},
    text::parse_retry_from_error_text,
};

#[must_use]
pub fn extract_retry_after_from_body(body: &[u8]) -> Option<u64> {
    let Ok(value) = serde_json::from_slice::<Value>(body) else {
        return parse_retry_from_error_text(std::str::from_utf8(body).ok()?);
    };
    retry_after_from_json(&value).or_else(|| {
        parse_retry_from_error_text(&value.to_string())
            .or_else(|| parse_try_again_in_seconds(&value.to_string()))
    })
}

#[must_use]
pub fn upstream_hint_secs(body: Option<&[u8]>) -> Option<u64> {
    body.and_then(extract_retry_after_from_body)
}

#[must_use]
pub fn resolve_429_base_secs(
    body: Option<&[u8]>,
    header_secs: Option<u64>,
    failure_kind: FailureKind,
    rate_limit_fallback: u64,
    quota_exhausted_fallback: u64,
) -> u64 {
    let text_reset = body.and_then(|bytes| {
        parse_retry_from_error_text(std::str::from_utf8(bytes).unwrap_or(""))
    });
    let body_hint = upstream_hint_secs(body);

    match failure_kind {
        FailureKind::QuotaExhausted => text_reset
            .or(header_secs
                .filter(|secs| *secs >= quota_exhausted_fallback / 2))
            .or(body_hint.filter(|secs| *secs >= quota_exhausted_fallback / 2))
            .unwrap_or(quota_exhausted_fallback),
        FailureKind::RateLimit => header_secs
            .or(body_hint)
            .or(text_reset)
            .unwrap_or(rate_limit_fallback),
    }
}

fn retry_after_from_json(value: &Value) -> Option<u64> {
    match value {
        Value::Object(map) => {
            if let Some(delay) = map.get("retryDelay").and_then(Value::as_str) {
                return parse_retry_delay_seconds(delay);
            }
            if let Some(delay) =
                map.get("retry_after").and_then(parse_json_number)
            {
                return Some(delay);
            }
            for nested in map.values() {
                if let Some(delay) = retry_after_from_json(nested) {
                    return Some(delay);
                }
            }
        }
        Value::Array(items) => {
            for item in items {
                if let Some(delay) = retry_after_from_json(item) {
                    return Some(delay);
                }
            }
        }
        _ => {}
    }
    None
}

fn parse_json_number(value: &Value) -> Option<u64> {
    value.as_u64().or_else(|| {
        value
            .as_f64()
            .and_then(super::duration::finite_secs_f64_to_u64)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_gemini_retry_info_from_body() {
        let body = br#"{
            "error": {
                "code": 429,
                "status": "RESOURCE_EXHAUSTED",
                "details": [{
                    "@type": "type.googleapis.com/google.rpc.RetryInfo",
                    "retryDelay": "15.002899939s"
                }]
            }
        }"#;
        assert_eq!(extract_retry_after_from_body(body), Some(16));
    }

    #[test]
    fn parses_groq_error_json() {
        let body = br#"{"error":{"message":"Please try again in 12.5s"}}"#;
        assert_eq!(extract_retry_after_from_body(body), Some(13));
    }
}
