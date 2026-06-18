use chrono::{DateTime, Utc};
use http::HeaderMap;

use super::constants::cap_duration_secs;

#[must_use]
pub fn extract_retry_after_from_headers(headers: &HeaderMap) -> Option<u64> {
    let value = headers.get(http::header::RETRY_AFTER)?.to_str().ok()?;
    parse_retry_after_header(value).map(cap_duration_secs)
}

/// `OpenRouter` per-model daily quota reset (`X-RateLimit-Reset` epoch ms).
#[must_use]
pub fn extract_rate_limit_reset_secs(headers: &HeaderMap) -> Option<u64> {
    let value = headers.get("x-ratelimit-reset")?.to_str().ok()?;
    let reset_ms = value.parse::<u64>().ok()?;
    let now_ms = millis_to_u64(chrono::Utc::now().timestamp_millis());
    if reset_ms <= now_ms {
        return Some(0);
    }
    let delta_secs = (reset_ms - now_ms).div_ceil(1000);
    Some(cap_duration_secs(delta_secs))
}

/// `OmniRoute` `parseRetryAfter`: Groq `60s`/`5m`/`2h` before plain integer
/// parse.
#[must_use]
pub fn parse_retry_after_header(value: &str) -> Option<u64> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Some(secs) = parse_relative_unit(trimmed) {
        return Some(secs);
    }
    if let Ok(secs) = trimmed.parse::<u64>() {
        return Some(secs);
    }
    if let Ok(dt) =
        DateTime::parse_from_str(trimmed, "%a, %d %b %Y %H:%M:%S GMT")
    {
        let now = u64::try_from(Utc::now().timestamp()).unwrap_or(0);
        let target = u64::try_from(dt.timestamp()).ok()?;
        return Some(target.saturating_sub(now));
    }
    None
}

pub(crate) fn millis_to_u64(value: i64) -> u64 {
    u64::try_from(value.max(0)).unwrap_or(0)
}

fn parse_relative_unit(value: &str) -> Option<u64> {
    let lower = value.to_ascii_lowercase();
    if let Some(num) = lower.strip_suffix('s') {
        return num.parse::<u64>().ok();
    }
    if let Some(num) = lower.strip_suffix('m') {
        return num.parse::<u64>().ok().map(|m| m.saturating_mul(60));
    }
    if let Some(num) = lower.strip_suffix('h') {
        return num.parse::<u64>().ok().map(|h| h.saturating_mul(3600));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_integer_seconds() {
        assert_eq!(parse_retry_after_header("60"), Some(60));
        assert_eq!(parse_retry_after_header("0"), Some(0));
    }

    #[test]
    fn parses_groq_relative_units_before_integer_trap() {
        assert_eq!(parse_retry_after_header("60s"), Some(60));
        assert_eq!(parse_retry_after_header("5m"), Some(300));
        assert_eq!(parse_retry_after_header("2h"), Some(7200));
        assert_eq!(parse_retry_after_header("1H"), Some(3600));
    }

    #[test]
    fn rejects_unparseable_values() {
        assert_eq!(parse_retry_after_header(""), None);
        assert_eq!(parse_retry_after_header("60xyz"), None);
    }

    #[test]
    fn retry_after_takes_precedence_over_reset_for_rpm_path() {
        let future_ms =
            millis_to_u64(chrono::Utc::now().timestamp_millis()) + 90_000;
        let mut headers = HeaderMap::new();
        headers.insert(
            http::header::RETRY_AFTER,
            http::HeaderValue::from_static("45"),
        );
        headers.insert(
            "x-ratelimit-reset",
            http::HeaderValue::from_str(&future_ms.to_string()).unwrap(),
        );
        assert_eq!(extract_retry_after_from_headers(&headers), Some(45));
        assert!(extract_rate_limit_reset_secs(&headers).is_some());
    }

    #[test]
    fn parses_rate_limit_reset_epoch_ms() {
        let future_ms =
            millis_to_u64(chrono::Utc::now().timestamp_millis()) + 90_000;
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-ratelimit-reset",
            http::HeaderValue::from_str(&future_ms.to_string()).unwrap(),
        );
        let secs = extract_rate_limit_reset_secs(&headers).expect("reset");
        assert!((85..=95).contains(&secs));
    }
}
