use indexmap::IndexMap;
use serde::Deserialize;

use crate::types::provider::InferenceProvider;

const PROVIDER_LADDERS_YAML: &str =
    include_str!("../../config/embedded/provider-ladders.yaml");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LadderBand {
    Fast,
    Capacity,
    Stability,
}

impl LadderBand {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Fast => "fast",
            Self::Capacity => "capacity",
            Self::Stability => "stability",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LadderPosition {
    pub band: LadderBand,
    pub band_index: u16,
    pub position: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelLadderRegistry {
    providers: IndexMap<InferenceProvider, IndexMap<String, TierLadder>>,
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
struct LadderCatalog {
    #[serde(flatten)]
    providers: IndexMap<InferenceProvider, IndexMap<String, TierLadder>>,
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
struct TierLadder {
    #[serde(default)]
    fast: Vec<String>,
    #[serde(default)]
    capacity: Vec<String>,
    #[serde(default)]
    stability: Vec<String>,
}

impl Default for ModelLadderRegistry {
    fn default() -> Self {
        let catalog: LadderCatalog = serde_yml::from_str(PROVIDER_LADDERS_YAML)
            .expect("embedded provider-ladders.yaml must parse");
        Self {
            providers: catalog.providers,
        }
    }
}

impl ModelLadderRegistry {
    #[must_use]
    pub fn position(
        &self,
        provider: &InferenceProvider,
        tier: &str,
        model: &str,
    ) -> Option<LadderPosition> {
        let tier_ladder = self.providers.get(provider)?.get(tier)?;
        let slug =
            crate::config::catalog_limit_resolve::normalize_model_slug(model);
        for (band_index, (band, models)) in [
            (LadderBand::Fast, &tier_ladder.fast),
            (LadderBand::Capacity, &tier_ladder.capacity),
            (LadderBand::Stability, &tier_ladder.stability),
        ]
        .into_iter()
        .enumerate()
        {
            if let Some(position) = models.iter().position(|m| m == &slug) {
                return Some(LadderPosition {
                    band,
                    band_index: u16::try_from(band_index).unwrap_or(u16::MAX),
                    position: u16::try_from(position).unwrap_or(u16::MAX),
                });
            }
        }
        None
    }

    #[must_use]
    pub fn ladder_model_slugs(
        &self,
        provider: &InferenceProvider,
        tier: &str,
    ) -> Vec<String> {
        let Some(tier_ladder) =
            self.providers.get(provider).and_then(|t| t.get(tier))
        else {
            return Vec::new();
        };
        tier_ladder
            .fast
            .iter()
            .chain(&tier_ladder.capacity)
            .chain(&tier_ladder.stability)
            .cloned()
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gemini_free_ladder_order() {
        let registry = ModelLadderRegistry::default();
        let provider = InferenceProvider::GoogleGemini;
        let flash = registry
            .position(&provider, "free", "gemini-3-flash-preview")
            .expect("flash");
        let lite = registry
            .position(&provider, "free", "gemini-3.1-flash-lite")
            .expect("lite");
        let pro = registry
            .position(&provider, "free", "gemini-2.5-pro")
            .expect("pro");
        assert_eq!(flash.band, LadderBand::Fast);
        assert_eq!(lite.band, LadderBand::Capacity);
        assert_eq!(pro.band, LadderBand::Stability);
        assert!(flash.band_index < lite.band_index);
        assert!(lite.band_index < pro.band_index);
    }

    #[test]
    fn groq_has_no_ladder_entry() {
        let registry = ModelLadderRegistry::default();
        let provider = InferenceProvider::Named("groq".into());
        assert!(
            registry
                .position(&provider, "free", "llama-3.3-70b-versatile")
                .is_none()
        );
    }

    #[test]
    fn every_ladder_slug_resolves_in_catalog() {
        use crate::config::catalog_limit_resolve::catalog_limit_resolve;

        let registry = ModelLadderRegistry::default();
        let catalog =
            crate::config::provider_limits::ProviderLimitCatalog::default();
        let provider = InferenceProvider::GoogleGemini;
        for slug in registry.ladder_model_slugs(&provider, "free") {
            catalog_limit_resolve(&catalog, &provider, "free", &slug)
                .unwrap_or_else(|| {
                    panic!("ladder slug {slug} must resolve in catalog")
                });
        }
    }
}
