use std::fmt;

use derive_more::{AsRef, Deref};
use indexmap::IndexMap;
use serde::{
    Deserialize, Deserializer, Serialize, Serializer,
    de::{self, Visitor},
};

use crate::types::provider::InferenceProvider;

const PROVIDER_LIMITS_YAML: &str =
    include_str!("../../config/embedded/provider-limits.yaml");

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, AsRef, Deref)]
pub struct ProviderLimitCatalog(
    pub(crate) IndexMap<InferenceProvider, ProviderLimitConfig>,
);

impl Default for ProviderLimitCatalog {
    fn default() -> Self {
        serde_yml::from_str(PROVIDER_LIMITS_YAML)
            .expect("Always valid if tests pass")
    }
}

impl ProviderLimitCatalog {
    #[must_use]
    pub fn provider(
        &self,
        provider: &InferenceProvider,
    ) -> Option<&ProviderLimitConfig> {
        self.0.get(provider)
    }

    #[must_use]
    pub fn model(
        &self,
        provider: &InferenceProvider,
        tier: &str,
        model: &str,
    ) -> Option<&QuotaSubjectLimits> {
        self.provider(provider)?.tier(tier)?.model(model)
    }

    #[must_use]
    pub fn endpoint_model(
        &self,
        provider: &InferenceProvider,
        tier: &str,
        endpoint: &str,
        model: &str,
    ) -> Option<&QuotaSubjectLimits> {
        self.provider(provider)?
            .tier(tier)?
            .endpoint_model(endpoint, model)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "kebab-case")]
pub struct ProviderLimitConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub observed_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<String>,
    #[serde(skip_serializing_if = "IndexMap::is_empty")]
    pub runtime_sources: IndexMap<String, RuntimeLimitSource>,
    pub tiers: IndexMap<String, ProviderLimitTier>,
}

impl Default for ProviderLimitConfig {
    fn default() -> Self {
        Self {
            observed_at: None,
            scope: None,
            source: None,
            notes: Vec::new(),
            runtime_sources: IndexMap::new(),
            tiers: IndexMap::new(),
        }
    }
}

impl ProviderLimitConfig {
    #[must_use]
    pub fn tier(&self, tier: &str) -> Option<&ProviderLimitTier> {
        self.tiers.get(tier)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "kebab-case")]
pub struct ProviderLimitTier {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub billing: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub qualification: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<String>,
    pub limits: QuotaLimits,
    #[serde(skip_serializing_if = "IndexMap::is_empty")]
    pub rules: IndexMap<String, LimitRule>,
    #[serde(skip_serializing_if = "IndexMap::is_empty")]
    pub models: IndexMap<String, QuotaSubjectLimits>,
    #[serde(skip_serializing_if = "IndexMap::is_empty")]
    pub tools: IndexMap<String, IndexMap<String, QuotaSubjectLimits>>,
    #[serde(skip_serializing_if = "IndexMap::is_empty")]
    pub endpoints: IndexMap<String, EndpointLimitConfig>,
}

impl Default for ProviderLimitTier {
    fn default() -> Self {
        Self {
            display_name: None,
            billing: None,
            qualification: None,
            notes: Vec::new(),
            limits: QuotaLimits::default(),
            rules: IndexMap::new(),
            models: IndexMap::new(),
            tools: IndexMap::new(),
            endpoints: IndexMap::new(),
        }
    }
}

impl ProviderLimitTier {
    #[must_use]
    pub fn model(&self, model: &str) -> Option<&QuotaSubjectLimits> {
        self.models.get(model).or_else(|| {
            self.endpoints
                .values()
                .find_map(|endpoint| endpoint.models.get(model))
        })
    }

    #[must_use]
    pub fn endpoint_model(
        &self,
        endpoint: &str,
        model: &str,
    ) -> Option<&QuotaSubjectLimits> {
        self.endpoints.get(endpoint)?.models.get(model)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "kebab-case")]
pub struct EndpointLimitConfig {
    #[serde(skip_serializing_if = "IndexMap::is_empty")]
    pub models: IndexMap<String, QuotaSubjectLimits>,
}

impl Default for EndpointLimitConfig {
    fn default() -> Self {
        Self {
            models: IndexMap::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "kebab-case")]
pub struct RuntimeLimitSource {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub method: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "IndexMap::is_empty")]
    pub headers: IndexMap<String, String>,
    #[serde(skip_serializing_if = "IndexMap::is_empty")]
    pub fields: IndexMap<String, String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub deprecated_fields: Vec<String>,
}

impl Default for RuntimeLimitSource {
    fn default() -> Self {
        Self {
            method: None,
            url: None,
            headers: IndexMap::new(),
            fields: IndexMap::new(),
            deprecated_fields: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "kebab-case")]
pub struct LimitRule {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_suffix: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub condition: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<String>,
    pub limits: QuotaLimits,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "kebab-case")]
pub struct QuotaSubjectLimits {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tier: Option<String>,
    pub limits: QuotaLimits,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "kebab-case")]
pub struct QuotaLimits {
    pub rpm: QuotaValue,
    pub rpd: QuotaValue,
    pub tpm: QuotaValue,
    pub tpd: QuotaValue,
    pub audio_seconds_per_hour: QuotaValue,
    pub audio_seconds_per_day: QuotaValue,
    pub monthly_usd: QuotaValue,
}

impl Default for QuotaLimits {
    fn default() -> Self {
        Self {
            rpm: QuotaValue::Unknown,
            rpd: QuotaValue::Unknown,
            tpm: QuotaValue::Unknown,
            tpd: QuotaValue::Unknown,
            audio_seconds_per_hour: QuotaValue::Unknown,
            audio_seconds_per_day: QuotaValue::Unknown,
            monthly_usd: QuotaValue::Unknown,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum QuotaValue {
    Limited(u64),
    Unlimited,
    #[default]
    Unknown,
}

impl Serialize for QuotaValue {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            QuotaValue::Limited(value) => serializer.serialize_u64(*value),
            QuotaValue::Unlimited => serializer.serialize_str("unlimited"),
            QuotaValue::Unknown => serializer.serialize_str("unknown"),
        }
    }
}

impl<'de> Deserialize<'de> for QuotaValue {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(QuotaValueVisitor)
    }
}

struct QuotaValueVisitor;

impl Visitor<'_> for QuotaValueVisitor {
    type Value = QuotaValue;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a non-negative integer, unlimited, or unknown")
    }

    fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E> {
        Ok(QuotaValue::Limited(value))
    }

    fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        u64::try_from(value)
            .map(QuotaValue::Limited)
            .map_err(|_| E::custom("quota value cannot be negative"))
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        match value.trim().to_ascii_lowercase().as_str() {
            "unlimited" | "no-limit" | "no limit" => Ok(QuotaValue::Unlimited),
            "unknown" | "-" => Ok(QuotaValue::Unknown),
            value => {
                value.parse::<u64>().map(QuotaValue::Limited).map_err(|_| {
                    E::unknown_variant(value, &["unlimited", "unknown"])
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_provider_limit_catalog_loads_from_yaml() {
        let catalog = ProviderLimitCatalog::default();

        assert!(catalog.provider(&InferenceProvider::GoogleGemini).is_some());
        assert!(
            catalog
                .provider(&InferenceProvider::Named("groq".into()))
                .is_some()
        );
        assert!(
            catalog
                .provider(&InferenceProvider::GoogleGemini)
                .unwrap()
                .tier("free")
                .is_some()
        );
        assert!(
            catalog
                .provider(&InferenceProvider::Named("groq".into()))
                .unwrap()
                .tier("developer")
                .is_some()
        );
    }

    #[test]
    fn catalog_contains_gemini_text_model_limits() {
        let catalog = ProviderLimitCatalog::default();
        let limits = &catalog
            .model(&InferenceProvider::GoogleGemini, "free", "gemini-2.0-flash")
            .unwrap()
            .limits;

        assert_eq!(limits.rpm, QuotaValue::Limited(30_000));
        assert_eq!(limits.tpm, QuotaValue::Limited(30_000_000));
        assert_eq!(limits.rpd, QuotaValue::Unlimited);
        assert_eq!(limits.tpd, QuotaValue::Unknown);
    }

    #[test]
    fn catalog_contains_groq_model_limits() {
        let catalog = ProviderLimitCatalog::default();
        let limits = &catalog
            .model(
                &InferenceProvider::Named("groq".into()),
                "developer",
                "llama-3.1-8b-instant",
            )
            .unwrap()
            .limits;

        assert_eq!(limits.rpm, QuotaValue::Limited(30));
        assert_eq!(limits.rpd, QuotaValue::Limited(14_400));
        assert_eq!(limits.tpm, QuotaValue::Limited(6_000));
        assert_eq!(limits.tpd, QuotaValue::Limited(500_000));
    }

    #[test]
    fn catalog_contains_groq_speech_to_text_limits() {
        let catalog = ProviderLimitCatalog::default();
        let limits = &catalog
            .endpoint_model(
                &InferenceProvider::Named("groq".into()),
                "developer",
                "speech-to-text",
                "whisper-large-v3",
            )
            .unwrap()
            .limits;

        assert_eq!(limits.rpm, QuotaValue::Limited(20));
        assert_eq!(limits.rpd, QuotaValue::Limited(2_000));
        assert_eq!(limits.audio_seconds_per_hour, QuotaValue::Limited(7_200));
        assert_eq!(limits.audio_seconds_per_day, QuotaValue::Limited(28_800));
    }

    #[test]
    fn catalog_contains_openrouter_dynamic_key_source_and_free_rules() {
        let catalog = ProviderLimitCatalog::default();
        let provider =
            catalog.provider(&InferenceProvider::OpenRouter).unwrap();
        let key_info = provider.runtime_sources.get("key-info").unwrap();

        assert_eq!(
            key_info.url.as_deref(),
            Some("https://openrouter.ai/api/v1/key")
        );
        assert_eq!(
            key_info.fields.get("is-free-tier").unwrap(),
            "Whether the user has paid for credits before."
        );

        let free_rule = provider
            .tier("free")
            .unwrap()
            .rules
            .get("free-model-variants")
            .unwrap();
        assert_eq!(free_rule.model_suffix.as_deref(), Some(":free"));
        assert_eq!(free_rule.limits.rpm, QuotaValue::Limited(20));
        assert_eq!(free_rule.limits.rpd, QuotaValue::Limited(50));

        let paid_rule = provider
            .tier("paid-credits")
            .unwrap()
            .rules
            .get("free-model-variants")
            .unwrap();
        assert_eq!(paid_rule.limits.rpm, QuotaValue::Limited(20));
        assert_eq!(paid_rule.limits.rpd, QuotaValue::Limited(1_000));
    }

    #[test]
    fn catalog_contains_openai_usage_tiers_and_runtime_headers() {
        let catalog = ProviderLimitCatalog::default();
        let provider = catalog.provider(&InferenceProvider::OpenAI).unwrap();
        let headers = &provider
            .runtime_sources
            .get("response-headers")
            .unwrap()
            .headers;

        assert_eq!(
            headers.get("remaining-tokens").unwrap(),
            "x-ratelimit-remaining-tokens"
        );
        assert_eq!(
            provider.tier("tier-5").unwrap().limits.monthly_usd,
            QuotaValue::Limited(200_000)
        );
    }

    #[test]
    fn quota_value_deserializes_common_markers() {
        assert_eq!(
            serde_yml::from_str::<QuotaValue>("unlimited").unwrap(),
            QuotaValue::Unlimited
        );
        assert_eq!(
            serde_yml::from_str::<QuotaValue>("unknown").unwrap(),
            QuotaValue::Unknown
        );
        assert_eq!(
            serde_yml::from_str::<QuotaValue>("42").unwrap(),
            QuotaValue::Limited(42)
        );
    }
}
