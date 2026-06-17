use serde::{Deserialize, Serialize};

use crate::{
    config::providers::ProvidersConfig,
    types::{model_id::ModelId, provider::InferenceProvider},
};

#[derive(Debug, Clone, Default, Deserialize, Serialize, Eq, PartialEq)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct ModelCapabilityConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_window: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supports_tools: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supports_json_schema: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supports_vision: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<bool>,
}

/// Same capability resolution as the router (embedded `model-capabilities` yaml
/// + provider defaults).
#[must_use]
pub fn supports_json_schema(
    providers: &ProvidersConfig,
    provider: &InferenceProvider,
    model: &str,
) -> bool {
    let Ok(model_id) = ModelId::from_str_and_provider(provider.clone(), model)
    else {
        return false;
    };
    let metadata = providers
        .get(provider)
        .and_then(|cfg| cfg.model_capabilities.get(&model_id));
    crate::router::capability::get_model_capability(
        provider, &model_id, metadata,
    )
    .supports_json_schema
}
