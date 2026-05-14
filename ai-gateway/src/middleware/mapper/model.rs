use std::sync::Arc;

use derive_more::{AsRef, Deref, DerefMut};
use rustc_hash::{FxHashMap as HashMap, FxHashSet as HashSet};

use crate::{
    app_state::AppState,
    config::{model_mapping::ModelMappingConfig, router::RouterConfig},
    error::mapper::MapperError,
    types::{
        model_id::{ModelId, ModelIdWithoutVersion, ModelName},
        provider::InferenceProvider,
    },
};

#[derive(Debug, Clone, Eq, PartialEq, Deref, DerefMut, AsRef)]
struct ProviderModels(
    HashMap<InferenceProvider, HashSet<ModelIdWithoutVersion>>,
);

impl ProviderModels {
    fn new(app_state: &AppState) -> Self {
        let mut map = HashMap::default();
        for (provider, config) in app_state.config().providers.iter() {
            let models =
                config.models.iter().map(|m| m.clone().into()).collect();
            map.insert(provider.clone(), models);
        }
        Self(map)
    }
}

#[derive(Debug, Clone)]
pub struct ModelMapper {
    app_state: AppState,
    router_config: Option<Arc<RouterConfig>>,
    model_id: Option<ModelId>,
    provider_models: ProviderModels,
}

impl ModelMapper {
    #[must_use]
    pub fn new_for_router(
        app_state: AppState,
        router_config: Arc<RouterConfig>,
    ) -> Self {
        let provider_models = ProviderModels::new(&app_state);
        Self {
            app_state,
            router_config: Some(router_config),
            model_id: None,
            provider_models,
        }
    }

    #[must_use]
    pub fn new_with_model_id(
        app_state: AppState,
        router_config: Arc<RouterConfig>,
        model_id: ModelId,
    ) -> Self {
        let provider_models = ProviderModels::new(&app_state);
        Self {
            app_state,
            router_config: Some(router_config),
            model_id: Some(model_id),
            provider_models,
        }
    }

    #[must_use]
    pub fn new(app_state: AppState) -> Self {
        let provider_models = ProviderModels::new(&app_state);
        Self {
            app_state,
            router_config: None,
            model_id: None,
            provider_models,
        }
    }

    fn default_model_mapping(&self) -> &ModelMappingConfig {
        &self.app_state.0.config.default_model_mapping
    }

    /// Map a model to a new model name for a target provider.
    ///
    /// If the source model is offered by the target provider, return the source
    /// model name. Otherwise, use the model mapping from router config.
    /// If that doesn't have a mapping, use the default model mapping from the
    /// global config. (maybe we should put usage of the default mapping
    /// behind a flag so its up to the user,  although declaring mappings
    /// for _every_ model seems onerous)
    pub fn map_model(
        &self,
        source_model: &ModelId,
        target_provider: &InferenceProvider,
    ) -> Result<ModelId, MapperError> {
        // this model id comes from the router's configuration, e.g. weighted
        // model configuration
        if let Some(model_id) = self.model_id.clone() {
            return Ok(model_id);
        }
        let models_offered_by_target_provider =
            self.provider_models.0.get(target_provider).ok_or_else(|| {
                MapperError::NoProviderConfig(target_provider.clone())
            })?;

        let source_model_w_out_version =
            ModelIdWithoutVersion::from(source_model.clone());

        if models_offered_by_target_provider
            .contains(&source_model_w_out_version)
        {
            return Ok(source_model.clone());
        }

        if let Some(openrouter_model) =
            Self::openrouter_upstream_model(source_model, target_provider)
            && models_offered_by_target_provider.contains(
                &ModelIdWithoutVersion::from(openrouter_model.clone()),
            )
        {
            return Ok(openrouter_model);
        }

        let model_mapping_config = if let Some(router_model_mapping) =
            self.router_config.as_ref().and_then(|c| c.model_mappings())
        {
            router_model_mapping
        } else {
            self.default_model_mapping()
        };

        let source_model_name = ModelName::from_model(source_model);
        let possible_mappings = model_mapping_config
            .as_ref()
            .get(&source_model_name)
            .ok_or_else(|| {
                MapperError::NoModelMapping(
                    target_provider.clone(),
                    source_model_name.as_ref().to_string(),
                )
            })?;

        // get the first model from the router model mapping that the target
        // provider supports
        let target_model = possible_mappings
            .iter()
            .find(|m| {
                let possible_mapping = (*m).clone().into();
                models_offered_by_target_provider.contains(&possible_mapping)
                    && m.inference_provider() == Some(target_provider.clone())
            })
            .ok_or_else(|| {
                MapperError::NoModelMapping(
                    target_provider.clone(),
                    source_model_name.as_ref().to_string(),
                )
            })?
            .clone();

        Ok(target_model)
    }

    /// Map an `InferenceProvider` to its OpenRouter upstream namespace.
    ///
    /// OpenRouter routes models by vendor namespace (the part before `/`
    /// in slugs like `openai/gpt-4o` or `x-ai/grok-4`). Most align with the
    /// provider's canonical name; a few don't and need explicit mapping:
    ///   - `Named("xai")`     → `"x-ai"`     (dash, not joined word)
    ///   - `Named("mistral")` → `"mistralai"` (no separator)
    ///
    /// Returns `None` for providers without a canonical OpenRouter namespace
    /// (`Bedrock`, `Ollama`, `OpenRouter` itself, unknown `Named`).
    fn openrouter_namespace(
        provider: &InferenceProvider,
    ) -> Option<&'static str> {
        match provider {
            InferenceProvider::OpenAI => Some("openai"),
            InferenceProvider::Anthropic => Some("anthropic"),
            InferenceProvider::GoogleGemini => Some("google"),
            InferenceProvider::Named(name) => match name.as_str() {
                "xai" => Some("x-ai"),
                "mistral" => Some("mistralai"),
                "deepseek" => Some("deepseek"),
                "meta-llama" => Some("meta-llama"),
                "qwen" => Some("qwen"),
                "z-ai" => Some("z-ai"),
                "moonshotai" => Some("moonshotai"),
                "nousresearch" => Some("nousresearch"),
                _ => None,
            },
            // Bedrock/Ollama are not on OpenRouter; OpenRouter itself is
            // the target, not a source upstream.
            _ => None,
        }
    }

    /// Build an OpenRouter slug for a source model, when the source
    /// provider has a known OpenRouter namespace.
    ///
    /// Generalised from the original OpenAI-only logic: previously only
    /// `source.provider == OpenAI` produced an `openai/{model}` slug,
    /// which made cascade-fallback through OpenRouter work for OpenAI but
    /// not for Anthropic/Gemini/Grok/etc. Now any provider in the
    /// `openrouter_namespace` table can fallback automatically.
    fn openrouter_upstream_model(
        source_model: &ModelId,
        target_provider: &InferenceProvider,
    ) -> Option<ModelId> {
        if target_provider != &InferenceProvider::OpenRouter {
            return None;
        }
        let source_provider = source_model.inference_provider()?;
        let namespace = Self::openrouter_namespace(&source_provider)?;
        ModelId::from_str_and_provider(
            InferenceProvider::OpenRouter,
            &format!("{namespace}/{source_model}"),
        )
        .ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn openrouter_namespace_built_in_providers() {
        assert_eq!(
            ModelMapper::openrouter_namespace(&InferenceProvider::OpenAI),
            Some("openai")
        );
        assert_eq!(
            ModelMapper::openrouter_namespace(&InferenceProvider::Anthropic),
            Some("anthropic")
        );
        assert_eq!(
            ModelMapper::openrouter_namespace(&InferenceProvider::GoogleGemini),
            Some("google")
        );
        assert_eq!(
            ModelMapper::openrouter_namespace(&InferenceProvider::OpenRouter),
            None,
            "OpenRouter is the target, not a source namespace"
        );
        assert_eq!(
            ModelMapper::openrouter_namespace(&InferenceProvider::Bedrock),
            None
        );
        assert_eq!(
            ModelMapper::openrouter_namespace(&InferenceProvider::Ollama),
            None
        );
    }

    #[test]
    fn openrouter_namespace_named_with_corrections() {
        assert_eq!(
            ModelMapper::openrouter_namespace(&InferenceProvider::Named(
                "xai".into()
            )),
            Some("x-ai"),
            "xai provider must map to x-ai (with dash) namespace on OpenRouter"
        );
        assert_eq!(
            ModelMapper::openrouter_namespace(&InferenceProvider::Named(
                "mistral".into()
            )),
            Some("mistralai"),
            "mistral provider must map to mistralai namespace on OpenRouter"
        );
    }

    #[test]
    fn openrouter_namespace_named_passthrough() {
        for vendor in [
            "deepseek",
            "meta-llama",
            "qwen",
            "z-ai",
            "moonshotai",
            "nousresearch",
        ] {
            assert_eq!(
                ModelMapper::openrouter_namespace(&InferenceProvider::Named(
                    vendor.into()
                )),
                Some(vendor),
                "{vendor} should passthrough as OpenRouter namespace"
            );
        }
    }

    #[test]
    fn openrouter_namespace_unknown_named_returns_none() {
        assert_eq!(
            ModelMapper::openrouter_namespace(&InferenceProvider::Named(
                "totally-unknown-vendor".into()
            )),
            None,
            "unknown vendors should fail closed (None), forcing explicit mapping"
        );
    }

    #[test]
    fn openrouter_upstream_model_openai_source() {
        let source = ModelId::from_str_and_provider(
            InferenceProvider::OpenAI,
            "gpt-4o-mini",
        )
        .unwrap();
        let result = ModelMapper::openrouter_upstream_model(
            &source,
            &InferenceProvider::OpenRouter,
        )
        .unwrap();
        let ModelId::ModelIdWithVersion { provider, id } = result else {
            panic!("Expected ModelIdWithVersion");
        };
        assert_eq!(provider, InferenceProvider::OpenRouter);
        assert_eq!(id.model, "openai/gpt-4o-mini");
    }

    #[test]
    fn openrouter_upstream_model_anthropic_source() {
        let source = ModelId::from_str_and_provider(
            InferenceProvider::Anthropic,
            "claude-3-5-haiku",
        )
        .unwrap();
        let result = ModelMapper::openrouter_upstream_model(
            &source,
            &InferenceProvider::OpenRouter,
        )
        .unwrap();
        let ModelId::ModelIdWithVersion { provider, id } = result else {
            panic!("Expected ModelIdWithVersion");
        };
        assert_eq!(provider, InferenceProvider::OpenRouter);
        assert_eq!(id.model, "anthropic/claude-3-5-haiku");
    }

    #[test]
    fn openrouter_upstream_model_xai_source_uses_corrected_namespace() {
        let source = ModelId::from_str_and_provider(
            InferenceProvider::Named("xai".into()),
            "grok-4",
        )
        .unwrap();
        let result = ModelMapper::openrouter_upstream_model(
            &source,
            &InferenceProvider::OpenRouter,
        )
        .unwrap();
        let ModelId::ModelIdWithVersion { provider, id } = result else {
            panic!("Expected ModelIdWithVersion");
        };
        assert_eq!(provider, InferenceProvider::OpenRouter);
        assert_eq!(
            id.model, "x-ai/grok-4",
            "xai source must produce x-ai/ namespace, not xai/"
        );
    }

    #[test]
    fn openrouter_upstream_model_mistral_source_uses_corrected_namespace() {
        let source = ModelId::from_str_and_provider(
            InferenceProvider::Named("mistral".into()),
            "mistral-large",
        )
        .unwrap();
        let result = ModelMapper::openrouter_upstream_model(
            &source,
            &InferenceProvider::OpenRouter,
        )
        .unwrap();
        let ModelId::ModelIdWithVersion { provider: _, id } = result else {
            panic!("Expected ModelIdWithVersion");
        };
        assert_eq!(
            id.model, "mistralai/mistral-large",
            "mistral source must produce mistralai/ namespace"
        );
    }

    #[test]
    fn openrouter_upstream_model_qwen_free_source_preserves_suffix() {
        let source = ModelId::from_str_and_provider(
            InferenceProvider::Named("qwen".into()),
            "qwen3-coder:free",
        )
        .unwrap();
        let result = ModelMapper::openrouter_upstream_model(
            &source,
            &InferenceProvider::OpenRouter,
        )
        .unwrap();
        let ModelId::ModelIdWithVersion { provider: _, id } = result else {
            panic!("Expected ModelIdWithVersion");
        };
        assert_eq!(
            id.model, "qwen/qwen3-coder:free",
            ":free suffix must survive namespace re-prefixing"
        );
    }

    #[test]
    fn openrouter_upstream_model_non_openrouter_target_returns_none() {
        let source =
            ModelId::from_str_and_provider(InferenceProvider::OpenAI, "gpt-4o")
                .unwrap();
        assert!(
            ModelMapper::openrouter_upstream_model(
                &source,
                &InferenceProvider::OpenAI,
            )
            .is_none(),
            "non-OpenRouter target must short-circuit"
        );
    }

    #[test]
    fn openrouter_upstream_model_unknown_named_source_returns_none() {
        let source = ModelId::from_str_and_provider(
            InferenceProvider::Named("totally-unknown".into()),
            "some-model",
        )
        .unwrap();
        assert!(
            ModelMapper::openrouter_upstream_model(
                &source,
                &InferenceProvider::OpenRouter,
            )
            .is_none(),
            "unknown source vendor must fail closed"
        );
    }
}
