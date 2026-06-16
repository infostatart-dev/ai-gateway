use std::time::Duration;

use crate::config::provider_limits::{
    ProviderLimitCatalog, QuotaLimits, QuotaValue,
};

/// Effective upstream pacing derived from `provider-limits.yaml` (Strategy
/// input).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PacingLimits {
    pub concurrent: usize,
    pub rpm: u32,
    pub min_interval: Duration,
    pub max_queue_wait: Duration,
}

impl PacingLimits {
    #[must_use]
    pub fn from_quota(limits: &QuotaLimits) -> Option<Self> {
        let rpm = match limits.rpm {
            QuotaValue::Limited(v) if v > 0 => u32::try_from(v).ok()?,
            _ => return None,
        };
        let concurrent = limits
            .concurrent
            .and_then(|v| usize::try_from(v).ok())
            .filter(|v| *v > 0)
            .unwrap_or(1);
        let min_interval = limits
            .min_interval_ms
            .map_or(Duration::ZERO, Duration::from_millis);
        Some(Self {
            concurrent,
            rpm,
            min_interval,
            max_queue_wait: Duration::from_mins(2),
        })
    }
}

impl ProviderLimitCatalog {
    /// First tier with an explicit RPM limit and optional session pacing
    /// fields.
    #[must_use]
    pub fn pacing_limits_for(
        &self,
        provider: &crate::types::provider::InferenceProvider,
    ) -> Option<PacingLimits> {
        let config = self.provider(provider)?;
        for tier in config.tiers.values() {
            if let Some(limits) = PacingLimits::from_quota(&tier.limits) {
                return Some(limits);
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        config::provider_limits::ProviderLimitCatalog,
        types::provider::InferenceProvider,
    };

    #[test]
    fn chatgpt_web_catalog_exposes_session_pacing() {
        let catalog = ProviderLimitCatalog::default();
        let provider = InferenceProvider::Named("chatgpt-web".into());
        let limits = catalog
            .pacing_limits_for(&provider)
            .expect("chatgpt-web pacing");
        assert_eq!(limits.concurrent, 1);
        assert_eq!(limits.rpm, 4);
        assert_eq!(limits.min_interval, Duration::from_millis(12000));
    }

    #[test]
    fn providers_without_rpm_tier_have_no_gate() {
        let catalog = ProviderLimitCatalog::default();
        let provider = InferenceProvider::OpenAI;
        assert!(catalog.pacing_limits_for(&provider).is_none());
    }
}
