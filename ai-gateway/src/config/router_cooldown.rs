use std::time::Duration;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct RouterCooldownConfig {
    #[serde(with = "humantime_serde", default = "default_provider_error")]
    pub provider_error: Duration,
    #[serde(with = "humantime_serde", default = "default_rate_limit")]
    pub rate_limit: Duration,
    #[serde(with = "humantime_serde", default = "default_quota_exhausted")]
    pub quota_exhausted: Duration,
    #[serde(with = "humantime_serde", default = "default_auth_error")]
    pub auth_error: Duration,
    #[serde(with = "humantime_serde", default = "default_retry_after_buffer")]
    pub retry_after_buffer: Duration,
    #[serde(with = "humantime_serde", default = "default_abuse_block")]
    pub abuse_block: Duration,
    #[serde(
        with = "humantime_serde",
        default = "default_credential_restriction"
    )]
    pub credential_restriction: Duration,
}

impl Default for RouterCooldownConfig {
    fn default() -> Self {
        Self {
            provider_error: default_provider_error(),
            rate_limit: default_rate_limit(),
            quota_exhausted: default_quota_exhausted(),
            auth_error: default_auth_error(),
            retry_after_buffer: default_retry_after_buffer(),
            abuse_block: default_abuse_block(),
            credential_restriction: default_credential_restriction(),
        }
    }
}

#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize,
)]
#[serde(default, rename_all = "kebab-case")]
pub struct ProviderCooldownOverrides {
    #[serde(
        default,
        with = "humantime_serde::option",
        skip_serializing_if = "Option::is_none"
    )]
    pub provider_error: Option<Duration>,
    #[serde(
        default,
        with = "humantime_serde::option",
        skip_serializing_if = "Option::is_none"
    )]
    pub rate_limit: Option<Duration>,
    #[serde(
        default,
        with = "humantime_serde::option",
        skip_serializing_if = "Option::is_none"
    )]
    pub quota_exhausted: Option<Duration>,
    #[serde(
        default,
        with = "humantime_serde::option",
        skip_serializing_if = "Option::is_none"
    )]
    pub auth_error: Option<Duration>,
    #[serde(
        default,
        with = "humantime_serde::option",
        skip_serializing_if = "Option::is_none"
    )]
    pub retry_after_buffer: Option<Duration>,
    #[serde(
        default,
        with = "humantime_serde::option",
        skip_serializing_if = "Option::is_none"
    )]
    pub abuse_block: Option<Duration>,
    #[serde(
        default,
        with = "humantime_serde::option",
        skip_serializing_if = "Option::is_none"
    )]
    pub credential_restriction: Option<Duration>,
}

impl ProviderCooldownOverrides {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.provider_error.is_none()
            && self.rate_limit.is_none()
            && self.quota_exhausted.is_none()
            && self.auth_error.is_none()
            && self.retry_after_buffer.is_none()
            && self.abuse_block.is_none()
            && self.credential_restriction.is_none()
    }
}

impl RouterCooldownConfig {
    #[must_use]
    pub fn merge(&self, overrides: &ProviderCooldownOverrides) -> Self {
        Self {
            provider_error: overrides
                .provider_error
                .unwrap_or(self.provider_error),
            rate_limit: overrides.rate_limit.unwrap_or(self.rate_limit),
            quota_exhausted: overrides
                .quota_exhausted
                .unwrap_or(self.quota_exhausted),
            auth_error: overrides.auth_error.unwrap_or(self.auth_error),
            retry_after_buffer: overrides
                .retry_after_buffer
                .unwrap_or(self.retry_after_buffer),
            abuse_block: overrides.abuse_block.unwrap_or(self.abuse_block),
            credential_restriction: overrides
                .credential_restriction
                .unwrap_or(self.credential_restriction),
        }
    }
}

pub(crate) const fn default_provider_error() -> Duration {
    Duration::from_secs(15)
}

pub(crate) const fn default_rate_limit() -> Duration {
    Duration::from_mins(1)
}

pub(crate) const fn default_quota_exhausted() -> Duration {
    Duration::from_hours(1)
}

pub(crate) const fn default_auth_error() -> Duration {
    Duration::from_mins(5)
}

pub(crate) const fn default_retry_after_buffer() -> Duration {
    Duration::from_secs(1)
}

pub(crate) const fn default_abuse_block() -> Duration {
    Duration::from_hours(2)
}

pub(crate) const fn default_credential_restriction() -> Duration {
    Duration::from_hours(2)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_applies_partial_provider_overrides() {
        let defaults = RouterCooldownConfig::default();
        let merged = defaults.merge(&ProviderCooldownOverrides {
            rate_limit: Some(Duration::from_secs(90)),
            ..ProviderCooldownOverrides::default()
        });

        assert_eq!(merged.provider_error, defaults.provider_error);
        assert_eq!(merged.rate_limit, Duration::from_secs(90));
    }

    #[test]
    fn default_includes_credential_restriction_tier() {
        let config = RouterCooldownConfig::default();
        assert_eq!(config.credential_restriction, Duration::from_hours(2));
    }
}
