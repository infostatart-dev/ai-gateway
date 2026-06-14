use chrono::{DateTime, Utc};

use super::{
    constants::cap_duration_secs,
    duration::{parse_decimal_seconds, parse_hms_groups_seconds},
};

#[must_use]
pub fn parse_retry_from_error_text(text: &str) -> Option<u64> {
    parse_iso_timestamp_seconds(text)
        .or_else(|| parse_hms_phrase(text, "reset after "))
        .or_else(|| parse_hms_phrase(text, "will reset after "))
        .or_else(|| parse_hms_phrase(text, "resets in "))
        .or_else(|| parse_hms_phrase(text, "reset in "))
        .or_else(|| parse_openai_retry_after_seconds(text))
        .map(cap_duration_secs)
}

fn parse_iso_timestamp_seconds(text: &str) -> Option<u64> {
    static ISO: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let re = ISO.get_or_init(|| {
        regex::Regex::new(
            r"(?i)(?:try again at|wait until|resets? at|available at|retry after)\s+(\d{4}-\d{2}-\d{2}[Tt ]\d{2}:\d{2}(?::\d{2})?(?:\.\d+)?(?:Z|[+-]\d{2}:?\d{2})?)",
        )
        .expect("valid iso retry regex")
    });
    let caps = re.captures(text)?;
    let raw = caps.get(1)?.as_str();
    let parsed = DateTime::parse_from_rfc3339(raw)
        .or_else(|_| DateTime::parse_from_str(raw, "%Y-%m-%dT%H:%M:%SZ"))
        .ok()?;
    let now = Utc::now().timestamp().max(0) as u64;
    let target = u64::try_from(parsed.timestamp()).ok()?;
    (target > now).then_some(target - now)
}

fn parse_hms_phrase(text: &str, marker: &str) -> Option<u64> {
    let lower = text.to_ascii_lowercase();
    let start = lower.find(marker)? + marker.len();
    let rest = text[start..].trim();
    let end = rest
        .find(|c: char| !matches!(c, '0'..='9' | 'h' | 'm' | 's' | 'H' | 'M' | 'S'))
        .unwrap_or(rest.len());
    parse_hms_groups_seconds(&rest[..end])
}

fn parse_openai_retry_after_seconds(text: &str) -> Option<u64> {
    static RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let re = RE.get_or_init(|| {
        regex::Regex::new(r"(?i)retry\s+after\s+(\d+(?:\.\d+)?)\s*s")
            .expect("valid openai retry regex")
    });
    let caps = re.captures(text)?;
    parse_decimal_seconds(caps.get(1)?.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_reset_after_compound_duration() {
        assert_eq!(
            parse_retry_from_error_text("reset after 2h7m23s"),
            Some(7643)
        );
    }

    #[test]
    fn parses_resets_in_antigravity_phrasing() {
        assert_eq!(
            parse_retry_from_error_text("Resets in 164h27m24s"),
            Some(592_044)
        );
    }

    #[test]
    fn parses_openai_retry_after_message() {
        assert_eq!(
            parse_retry_from_error_text("Please retry after 20s"),
            Some(20)
        );
    }
}
