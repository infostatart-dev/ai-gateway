use chrono::{DateTime, Utc};
use http::HeaderMap;

use super::constants::cap_duration_secs;

#[must_use]
pub fn extract_retry_after_from_headers(headers: &HeaderMap) -> Option<u64> {
    let value = headers.get(http::header::RETRY_AFTER)?.to_str().ok()?;
    parse_retry_after_header(value).map(cap_duration_secs)
}

/// OmniRoute `parseRetryAfter`: Groq `60s`/`5m`/`2h` before plain integer parse.
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
    if let Ok(dt) = DateTime::parse_from_str(trimmed, "%a, %d %b %Y %H:%M:%S GMT") {
        let now = Utc::now().timestamp().max(0) as u64;
        let target = u64::try_from(dt.timestamp()).ok()?;
        return Some(target.saturating_sub(now));
    }
    None
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
}
