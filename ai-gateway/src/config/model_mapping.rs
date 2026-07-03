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
    fn gpt_5_mini_default_mapping_prefers_vllm_then_curated_free_providers() {
        let config = ModelMappingConfig::default();
        let mappings = config
            .as_ref()
            .get(&ModelName::owned("gpt-5-mini".into()))
            .expect("gpt-5-mini mapping");

        let first = mappings.first();
        assert_eq!(
            first.inference_provider(),
            Some(InferenceProvider::Named("vllm".into()))
        );
        assert_eq!(first.to_string(), "am-thinking-awq");

        let longcat_pos = mappings
            .iter()
            .position(|model| {
                model.inference_provider()
                    == Some(InferenceProvider::Named("longcat".into()))
            })
            .expect("longcat entry");
        assert_eq!(
            longcat_pos, 1,
            "longcat must be the first fallback after local vllm"
        );

        let bazaarlink_pos = mappings
            .iter()
            .position(|model| {
                model.inference_provider()
                    == Some(InferenceProvider::Named("bazaarlink".into()))
            })
            .expect("bazaarlink entry");
        assert_eq!(
            bazaarlink_pos, 2,
            "bazaarlink must remain the first curated free fallback after \
             longcat"
        );

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
        assert_eq!(
            first_openrouter, "openrouter/free",
            "first openrouter fallback must be the free router slug"
        );
    }

    #[test]
    fn gpt_5_4_nano_default_mapping_prefers_vllm_then_curated_free_before_paid()
    {
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
        assert_eq!(*first, InferenceProvider::Named("vllm".into()));

        let longcat_pos = providers
            .iter()
            .position(|p| *p == InferenceProvider::Named("longcat".into()))
            .expect("longcat fallback");
        let anthropic_pos = providers
            .iter()
            .position(|p| *p == InferenceProvider::Anthropic)
            .expect("anthropic fallback");
        let bazaarlink_pos = providers
            .iter()
            .position(|p| *p == InferenceProvider::Named("bazaarlink".into()))
            .expect("bazaarlink entry");
        let deepseek_web_pos = providers
            .iter()
            .position(|p| *p == InferenceProvider::Named("deepseek-web".into()))
            .expect("deepseek-web fallback");
        assert_eq!(
            longcat_pos, 1,
            "longcat must be the first fallback after local vllm"
        );
        assert_eq!(
            bazaarlink_pos, 2,
            "bazaarlink must remain the first curated free fallback after \
             longcat"
        );
        assert!(
            bazaarlink_pos < deepseek_web_pos,
            "deepseek-web must follow the curated free fallback"
        );
        assert!(bazaarlink_pos < anthropic_pos);

        let first_model = mappings.first().to_string();
        assert!(
            first_model.contains("am-thinking-awq"),
            "first nano mapping must be local vllm, got {first_model}"
        );
    }
}
