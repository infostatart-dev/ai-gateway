use serde::{Deserialize, Serialize};
use url::Url;

use crate::types::secret::Secret;

mod deserialize;
#[cfg(test)]
mod tests;

#[derive(
    Default, Debug, Clone, Deserialize, Serialize, PartialEq, Eq, Hash,
)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub enum HeliconeFeatures {
    #[default]
    None,
    Auth,
    Observability,
    #[serde(rename = "__prompts")]
    Prompts,
    All,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct HeliconeConfig {
    #[serde(default = "default_api_key")]
    pub api_key: Secret<String>,
    #[serde(default = "default_base_url")]
    pub base_url: Url,
    #[serde(default = "default_websocket_url")]
    pub websocket_url: Url,
    #[serde(default)]
    pub features: HeliconeFeatures,
}

impl HeliconeConfig {
    pub fn is_auth_enabled(&self) -> bool {
        self.features != HeliconeFeatures::None
    }
    pub fn is_auth_disabled(&self) -> bool {
        self.features == HeliconeFeatures::None
    }
    pub fn is_observability_enabled(&self) -> bool {
        self.features == HeliconeFeatures::All
            || self.features == HeliconeFeatures::Observability
    }
    pub fn is_prompts_enabled(&self) -> bool {
        self.features == HeliconeFeatures::All
            || self.features == HeliconeFeatures::Prompts
    }
}

impl Default for HeliconeConfig {
    fn default() -> Self {
        Self {
            api_key: default_api_key(),
            base_url: default_base_url(),
            websocket_url: default_websocket_url(),
            features: HeliconeFeatures::None,
        }
    }
}

pub fn default_api_key() -> Secret<String> {
    Secret::from(
        std::env::var("HELICONE_CONTROL_PLANE_API_KEY")
            .unwrap_or("sk-helicone-...".to_string()),
    )
}
pub fn default_base_url() -> Url {
    "https://api.helicone.ai".parse().unwrap()
}
pub fn default_websocket_url() -> Url {
    "wss://api.helicone.ai/ws/v1/router/control-plane"
        .parse()
        .unwrap()
}

#[cfg(feature = "testing")]
impl crate::tests::TestDefault for HeliconeConfig {
    fn test_default() -> Self {
        Self {
            base_url: "http://localhost:8585".parse().unwrap(),
            websocket_url: "ws://localhost:8585/ws/v1/router/control-plane"
                .parse()
                .unwrap(),
            features: HeliconeFeatures::All,
            api_key: default_api_key(),
        }
    }
}
