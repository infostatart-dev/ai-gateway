use derive_more::AsRef;
use nonempty_collections::{NEMap, NEVec};
use serde::{Deserialize, Serialize};

use crate::types::model_id::{ModelId, ModelName};

const MODEL_MAPPING_YAML: &str =
    include_str!("../../config/embedded/model-mapping.yaml");

/// Ordered fallback lists per source model. YAML sequence order is preserved
/// (`NEVec`); do not use unordered sets here or budget routing picks candidates
/// arbitrarily.
#[derive(Debug, Clone, Deserialize, Serialize, AsRef, PartialEq, Eq)]
pub struct ModelMappingConfig(
    pub(crate) NEMap<ModelName<'static>, NEVec<ModelId>>,
);

impl Default for ModelMappingConfig {
    #[allow(clippy::too_many_lines)]
    fn default() -> Self {
        serde_yml::from_str(MODEL_MAPPING_YAML)
            .expect("Always valid if tests pass")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::provider::InferenceProvider;

    #[test]
    fn test_default_model_mapping_config_loads_from_yaml_string() {
        let _default_config = ModelMappingConfig::default();
        // just want to make sure we don't panic...
    }

    #[test]
    fn yaml_sequence_order_is_preserved() {
        let yaml = r"
gpt-5-mini:
  - openrouter/openai/gpt-oss-120b:free
  - openrouter/qwen/qwen3-next-80b-a3b-instruct:free
  - groq/meta-llama/llama-4-scout-17b-16e-instruct
";
        let config: ModelMappingConfig = serde_yml::from_str(yaml).unwrap();
        let mappings = config
            .as_ref()
            .get(&ModelName::owned("gpt-5-mini".into()))
            .expect("mapping entry");

        let providers: Vec<_> = mappings
            .iter()
            .map(|model| model.inference_provider().expect("provider"))
            .collect();
        assert_eq!(
            providers,
            [
                InferenceProvider::OpenRouter,
                InferenceProvider::OpenRouter,
                InferenceProvider::Named("groq".into()),
            ]
        );

        let first = mappings.first();
        assert_eq!(
            first.inference_provider(),
            Some(InferenceProvider::OpenRouter)
        );
        assert!(
            first.to_string().contains("gpt-oss-120b"),
            "first openrouter fallback must follow yaml order, got {}",
            first
        );
    }

    #[test]
    fn gpt_5_mini_default_mapping_prefers_gpt_oss_on_openrouter() {
        let config = ModelMappingConfig::default();
        let mappings = config
            .as_ref()
            .get(&ModelName::owned("gpt-5-mini".into()))
            .expect("gpt-5-mini mapping");

        let openrouter_models: Vec<_> = mappings
            .iter()
            .filter(|model| {
                model.inference_provider()
                    == Some(InferenceProvider::OpenRouter)
            })
            .map(|model| model.to_string())
            .collect();

        let first_openrouter =
            openrouter_models.first().expect("openrouter entry");
        assert!(
            first_openrouter.contains("gpt-oss-120b:free"),
            "first openrouter fallback must be gpt-oss-120b:free, got \
             {first_openrouter}"
        );
    }

    #[test]
    fn gpt_5_4_nano_default_mapping_prefers_free_openrouter_before_anthropic() {
        let config = ModelMappingConfig::default();
        let mappings = config
            .as_ref()
            .get(&ModelName::owned("gpt-5.4-nano".into()))
            .expect("gpt-5.4-nano mapping");

        let providers: Vec<_> = mappings
            .iter()
            .map(|model| model.inference_provider().expect("provider"))
            .collect();

        let first = providers.first().expect("first mapping");
        assert_eq!(*first, InferenceProvider::OpenRouter);

        let anthropic_pos = providers
            .iter()
            .position(|p| *p == InferenceProvider::Anthropic)
            .expect("anthropic fallback");
        let openrouter_pos = providers
            .iter()
            .position(|p| *p == InferenceProvider::OpenRouter)
            .expect("openrouter entry");
        assert!(openrouter_pos < anthropic_pos);

        let first_model = mappings.first().to_string();
        assert!(
            first_model.contains("gpt-oss-120b:free"),
            "first nano mapping must be free openrouter, got {first_model}"
        );
    }
}
