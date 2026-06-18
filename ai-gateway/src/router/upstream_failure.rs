//! Normalized upstream failure events (wire-agnostic).

use std::time::Duration;

use chrono::{DateTime, Utc};
use http::StatusCode;
use serde_json::Value;

use crate::config::router_cooldown::RouterCooldownConfig;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpstreamFailureKind {
    CredentialRestricted,
}

#[must_use]
pub fn parse_restricted_until_from_body(
    body: Option<&[u8]>,
) -> Option<DateTime<Utc>> {
    let bytes = body?;
    let value: Value = serde_json::from_slice(bytes).ok()?;
    if value.pointer("/error/code").and_then(Value::as_str)
        != Some("credential_restricted")
    {
        return None;
    }
    value
        .pointer("/error/restricted_until")
        .and_then(Value::as_str)
        .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
        .map(|dt| dt.with_timezone(&Utc))
}

#[must_use]
pub fn looks_like_credential_restricted(body: Option<&[u8]>) -> bool {
    let Some(bytes) = body else {
        return false;
    };
    let Ok(value) = serde_json::from_slice::<Value>(bytes) else {
        return false;
    };
    value.pointer("/error/code").and_then(Value::as_str)
        == Some("credential_restricted")
}

#[must_use]
pub fn credential_restriction_cooldown(
    restricted_until: Option<DateTime<Utc>>,
    config: &RouterCooldownConfig,
) -> Duration {
    if let Some(until) = restricted_until {
        let now = Utc::now();
        if until > now
            && let Ok(delta) = (until - now).to_std()
        {
            return delta + config.retry_after_buffer;
        }
    }
    config.credential_restriction + config.retry_after_buffer
}

#[must_use]
pub fn unix_secs_to_utc(secs: i64) -> DateTime<Utc> {
    DateTime::from_timestamp(secs, 0).unwrap_or_else(Utc::now)
}

#[must_use]
pub fn credential_restricted_http_status() -> StatusCode {
    StatusCode::FORBIDDEN
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_restricted_until_from_openai_error_body() {
        let body = br#"{"error":{"code":"credential_restricted","restricted_until":"2026-06-19T09:34:11Z"}}"#;
        let until =
            parse_restricted_until_from_body(Some(body.as_ref())).unwrap();
        assert_eq!(until.to_rfc3339(), "2026-06-19T09:34:11+00:00");
    }

    #[test]
    fn cooldown_uses_until_minus_now() {
        let until = Utc::now() + chrono::Duration::hours(2);
        let config = RouterCooldownConfig::default();
        let cooldown = credential_restriction_cooldown(Some(until), &config);
        assert!(cooldown >= Duration::from_secs(7199));
        assert!(cooldown <= Duration::from_secs(7202));
    }

    #[test]
    fn cooldown_falls_back_to_catalog_without_until() {
        let config = RouterCooldownConfig::default();
        let cooldown = credential_restriction_cooldown(None, &config);
        assert_eq!(
            cooldown,
            config.credential_restriction + config.retry_after_buffer
        );
    }

    #[test]
    fn cooldown_clamps_past_restricted_until_to_catalog() {
        let config = RouterCooldownConfig::default();
        let past = Utc::now() - chrono::Duration::hours(1);
        let cooldown = credential_restriction_cooldown(Some(past), &config);
        assert_eq!(
            cooldown,
            config.credential_restriction + config.retry_after_buffer
        );
    }

    #[test]
    fn looks_like_credential_restricted_openai_error_body() {
        let body = br#"{"error":{"code":"credential_restricted"}}"#;
        assert!(looks_like_credential_restricted(Some(body.as_ref())));
        assert!(!looks_like_credential_restricted(Some(b"{}")));
    }
}
