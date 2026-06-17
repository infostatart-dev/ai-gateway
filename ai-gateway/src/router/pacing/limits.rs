use std::time::Duration;

use crate::config::provider_limits::{
    ProviderLimitCatalog, ProviderLimitTier, QuotaLimits, QuotaValue,
};

/// Effective upstream pacing derived from `provider-limits.yaml` (Strategy
/// input).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PacingLimits {
    pub concurrent: usize,
    pub rpm: u32,
    pub tpm: Option<u32>,
    pub rpd: Option<u32>,
    pub tpd: Option<u32>,
    pub daily_reset_utc_hour: u8,
    pub min_interval: Duration,
    pub max_queue_wait: Duration,
}

impl PacingLimits {
    #[must_use]
    pub fn from_quota(limits: &QuotaLimits) -> Option<Self> {
        let rpm = match limits.rpm {
            QuotaValue::Limited(v) if v > 0 => u32::try_from(v).ok()?,
            _ => u32::MAX,
        };
        let tpm = quota_u32(&limits.tpm);
        let rpd = quota_u32(&limits.rpd);
        let tpd = quota_u32(&limits.tpd);
        if rpm == u32::MAX && tpm.is_none() && rpd.is_none() && tpd.is_none() {
            return None;
        }
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
            tpm,
            rpd,
            tpd,
            daily_reset_utc_hour: 0,
            min_interval,
            max_queue_wait: Duration::from_mins(2),
        })
    }

    #[must_use]
    pub fn has_rpm_limit(&self) -> bool {
        self.rpm != u32::MAX
    }
}

fn quota_u32(value: &QuotaValue) -> Option<u32> {
    match value {
        QuotaValue::Limited(v) => u32::try_from(*v).ok(),
        QuotaValue::Unlimited | QuotaValue::Unknown => None,
    }
}

fn tier_pacing_limits(tier: &ProviderLimitTier) -> Option<PacingLimits> {
    if let Some(limits) = PacingLimits::from_quota(&tier.limits) {
        return Some(limits);
    }
    tier.endpoints
        .get("chat-completions")
        .and_then(|endpoint| endpoint.models.get("all"))
        .and_then(|subject| PacingLimits::from_quota(&subject.limits))
}

impl ProviderLimitCatalog {
    /// First tier with an explicit pacing limit and optional session pacing
    /// fields.
    #[must_use]
    pub fn pacing_limits_for(
        &self,
        provider: &crate::types::provider::InferenceProvider,
    ) -> Option<PacingLimits> {
        let config = self.provider(provider)?;
        let daily_reset = config.daily_reset_utc_hour.unwrap_or(0);
        for tier in config.tiers.values() {
            if let Some(mut limits) = tier_pacing_limits(tier) {
                limits.daily_reset_utc_hour = daily_reset;
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
    fn deepseek_web_catalog_exposes_session_pacing() {
        let catalog = ProviderLimitCatalog::default();
        let provider = InferenceProvider::Named("deepseek-web".into());
        let limits = catalog
            .pacing_limits_for(&provider)
            .expect("deepseek-web pacing");
        assert_eq!(limits.concurrent, 1);
        assert_eq!(limits.rpm, 6);
        assert_eq!(limits.min_interval, Duration::from_millis(10000));
    }

    #[test]
    fn providers_without_rpm_tier_have_no_gate() {
        let catalog = ProviderLimitCatalog::default();
        let provider = InferenceProvider::OpenAI;
        assert!(catalog.pacing_limits_for(&provider).is_none());
    }

    #[test]
    fn cloudflare_catalog_exposes_rpd_only_gate() {
        let catalog = ProviderLimitCatalog::default();
        let provider = InferenceProvider::Named("cloudflare".into());
        let limits = catalog
            .pacing_limits_for(&provider)
            .expect("cloudflare rpd pacing");
        assert!(!limits.has_rpm_limit());
        assert_eq!(limits.rpd, Some(300));
    }

    #[test]
    fn cerebras_catalog_exposes_rpm_tpm_tpd() {
        let catalog = ProviderLimitCatalog::default();
        let provider = InferenceProvider::Named("cerebras".into());
        let limits = catalog
            .pacing_limits_for(&provider)
            .expect("cerebras pacing");
        assert_eq!(limits.rpm, 30);
        assert_eq!(limits.tpm, Some(60_000));
        assert_eq!(limits.tpd, Some(1_000_000));
    }
}
