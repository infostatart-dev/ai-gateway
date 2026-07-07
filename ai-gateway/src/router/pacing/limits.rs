//! Per-model multi-dimension pacing limits resolved from the embedded catalog.

use std::time::Duration;

use crate::{
    config::{
        catalog_limit_resolve::catalog_limit_resolve,
        provider_limits::{
            ProviderLimitCatalog, ProviderLimitTier, QuotaLimits, QuotaValue,
        },
    },
    types::provider::InferenceProvider,
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

    #[must_use]
    pub fn resolve_for_model(
        catalog: &ProviderLimitCatalog,
        provider: &InferenceProvider,
        tier: &str,
        model: &str,
    ) -> Option<Self> {
        let resolved = catalog_limit_resolve(catalog, provider, tier, model)?;
        let daily_reset = catalog
            .provider(provider)
            .and_then(|cfg| cfg.daily_reset_utc_hour)
            .unwrap_or(0);
        let mut limits = Self::from_quota(&resolved.limits)?;
        limits.daily_reset_utc_hour = daily_reset;
        Some(limits)
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
        provider: &InferenceProvider,
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
    fn vllm_catalog_exposes_single_concurrency_pacing() {
        let catalog = ProviderLimitCatalog::default();
        let provider = InferenceProvider::Named("vllm".into());
        let limits = catalog.pacing_limits_for(&provider).expect("vllm pacing");
        assert_eq!(limits.concurrent, 1);
        assert_eq!(limits.rpm, 60);
        assert_eq!(limits.min_interval, Duration::ZERO);
    }

    #[test]
    fn gemini_per_model_limits_differ_by_slug() {
        let catalog = ProviderLimitCatalog::default();
        let provider = InferenceProvider::GoogleGemini;
        let flash = PacingLimits::resolve_for_model(
            &catalog,
            &provider,
            "free",
            "gemini-3-flash-preview",
        )
        .expect("flash");
        let lite = PacingLimits::resolve_for_model(
            &catalog,
            &provider,
            "free",
            "gemini-3.1-flash-lite",
        )
        .expect("lite");
        assert_ne!(flash.rpd, lite.rpd);
    }

    #[test]
    fn github_models_low_and_high_tiers_resolve_separately() {
        let catalog = ProviderLimitCatalog::default();
        let provider = InferenceProvider::Named("github-models".into());
        let low = PacingLimits::resolve_for_model(
            &catalog,
            &provider,
            "free",
            "openai/gpt-4o-mini",
        )
        .expect("low-tier limits");
        let high = PacingLimits::resolve_for_model(
            &catalog,
            &provider,
            "free",
            "openai/gpt-4.1",
        )
        .expect("high-tier limits");

        assert_eq!(low.rpm, 15);
        assert_eq!(low.rpd, Some(150));
        assert_eq!(low.concurrent, 5);
        assert_eq!(high.rpm, 10);
        assert_eq!(high.rpd, Some(50));
        assert_eq!(high.concurrent, 2);
    }

    #[test]
    fn providers_without_rpm_tier_have_no_gate() {
        let catalog = ProviderLimitCatalog::default();
        let provider = InferenceProvider::OpenAI;
        assert!(catalog.pacing_limits_for(&provider).is_none());
    }
}
