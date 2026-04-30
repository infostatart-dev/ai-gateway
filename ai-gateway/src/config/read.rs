use std::collections::HashMap;
use std::path::PathBuf;
use url::Url;
use json_patch::merge;
use compact_str::CompactString;

use crate::{
    config::{
        balance::{BalanceConfig, BalanceConfigInner},
        router::RouterConfig,
        Config, Error, DEFAULT_CONFIG_PATH,
    },
    endpoints::EndpointType,
    types::{
        provider::{InferenceProvider, ProviderKey},
        router::RouterId,
        secret::Secret,
    },
};

impl Config {
    pub fn try_read(config_file_path: Option<PathBuf>) -> Result<Self, Box<Error>> {
        let mut default_config = serde_json::to_value(Self::default()).expect("default config is serializable");
        let mut builder = config::Config::builder();
        if let Some(path) = config_file_path { builder = builder.add_source(config::File::from(path)); }
        else if std::fs::exists(DEFAULT_CONFIG_PATH).unwrap_or_default() { builder = builder.add_source(config::File::from(PathBuf::from(DEFAULT_CONFIG_PATH))); }
        builder = builder.add_source(config::Environment::with_prefix("AI_GATEWAY").try_parsing(true).separator("__").convert_case(config::Case::Kebab));
        let input_config: serde_json::Value = builder.build().map_err(Error::from).map_err(Box::new)?.try_deserialize().map_err(Error::from).map_err(Box::new)?;
        merge(&mut default_config, &input_config);
        let mut config: Config = serde_path_to_error::deserialize(default_config).map_err(Error::from).map_err(Box::new)?;

        let autodefault_id = RouterId::Named(CompactString::new("autodefault"));
        if config.routers.contains_key(&autodefault_id) {
            return Err(Box::new(Error::ReservedRouterId("autodefault".into())));
        }

        let mut available_providers = vec![];
        for provider in [
            InferenceProvider::OpenAI,
            InferenceProvider::Anthropic,
            InferenceProvider::GoogleGemini,
            InferenceProvider::Named(CompactString::new("xai")),
            InferenceProvider::Named(CompactString::new("groq")),
            InferenceProvider::Bedrock,
        ] {
            if ProviderKey::from_env(&provider).is_some() {
                available_providers.push(provider);
            }
        }
        
        if available_providers.is_empty() {
            available_providers.push(InferenceProvider::OpenAI);
        }

        let mut nes_providers = nonempty_collections::NESet::new(available_providers.pop().unwrap());
        for p in available_providers {
            nes_providers.insert(p);
        }

        let autodefault_router = RouterConfig {
            load_balance: BalanceConfig(HashMap::from([(
                EndpointType::Chat,
                BalanceConfigInner::BalancedLatency { providers: nes_providers },
            )])),
            ..Default::default()
        };

        config.routers.as_mut().insert(autodefault_id, autodefault_router);

        if let Ok(k) = std::env::var("HELICONE_CONTROL_PLANE_API_KEY") { config.helicone.api_key = Secret::from(k); }
        if let Ok(reg) = std::env::var("AWS_REGION") && let Some(p) = config.providers.get_mut(&InferenceProvider::Bedrock) {
            let url = format!("https://bedrock-runtime.{reg}.amazonaws.com");
            p.base_url = Url::parse(&url).map_err(Error::UrlParse)?;
        }
        Ok(config)
    }
}
