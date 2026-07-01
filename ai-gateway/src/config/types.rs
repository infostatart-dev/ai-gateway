use compact_str::CompactString;
use config::ConfigError;
use displaydoc::Display;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::types::provider::InferenceProvider;

pub const ROUTER_ID_REGEX: &str = r"^[A-Za-z0-9_-]{1,12}$";
pub const DEFAULT_CONFIG_PATH: &str = "/etc/ai-gateway/config.yaml";

#[derive(Debug, Error, Display)]
pub enum Error {
    /// error collecting config sources: {0}
    Source(#[from] ConfigError),
    /// deserialization error for input config: {0}
    InputConfigDeserialization(#[from] serde_path_to_error::Error<ConfigError>),
    /// deserialization error for merged config: {0}
    MergedConfigDeserialization(
        #[from] serde_path_to_error::Error<serde_json::Error>,
    ),
    /// URL parsing error: {0}
    UrlParse(#[from] url::ParseError),
    /// reserved router ID used: {0}
    ReservedRouterId(String),
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq, Hash)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct MiddlewareConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache: Option<crate::config::cache::CacheConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rate_limit: Option<crate::config::rate_limit::RateLimitConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retries: Option<crate::config::retry::RetryConfig>,
}

impl Default for MiddlewareConfig {
    fn default() -> Self {
        Self {
            cache: Some(crate::config::cache::CacheConfig::default()),
            rate_limit: None,
            retries: None,
        }
    }
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields, rename_all = "kebab-case")]
pub struct Config {
    pub telemetry: telemetry::Config,
    pub server: crate::config::server::ServerConfig,
    pub minio: crate::config::minio::Config,
    pub database: crate::config::database::DatabaseConfig,
    pub dispatcher: crate::config::dispatcher::DispatcherConfig,
    pub discover: crate::config::discover::DiscoverConfig,
    pub response_headers:
        crate::config::response_headers::ResponseHeadersConfig,
    pub observability: crate::config::observability::ObservabilityConfig,
    pub deployment_target: crate::config::deployment_target::DeploymentTarget,
    pub client_access: crate::config::client_access::ClientAccessConfig,
    pub control_plane: crate::config::control_plane::ControlPlaneConfig,
    pub default_model_mapping: crate::config::model_mapping::ModelMappingConfig,
    pub helicone: crate::config::helicone::HeliconeConfig,
    pub providers: crate::config::providers::ProvidersConfig,
    pub provider_limits: crate::config::provider_limits::ProviderLimitCatalog,
    #[serde(skip)]
    pub credentials: crate::config::credentials::CredentialRegistry,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_store: Option<crate::config::cache::CacheStore>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rate_limit_store: Option<crate::config::rate_limit::RateLimitStore>,
    #[serde(default)]
    pub decision: crate::config::decision::DecisionEngineConfig,
    pub global: MiddlewareConfig,
    pub unified_api: MiddlewareConfig,
    pub routers: crate::config::router::RouterConfigs,
}

impl Default for Config {
    fn default() -> Self {
        let providers = crate::config::providers::ProvidersConfig::default();
        let mut secrets =
            crate::config::secrets_file::SecretsFile::load_discovered();
        let credentials = crate::config::credentials::CredentialRegistry::build(
            &providers,
            &mut secrets,
        );
        crate::config::secrets_file::SecretsFile::install(secrets);
        Self {
            telemetry: telemetry::Config::default(),
            server: crate::config::server::ServerConfig::default(),
            minio: crate::config::minio::Config::default(),
            database: crate::config::database::DatabaseConfig::default(),
            dispatcher: crate::config::dispatcher::DispatcherConfig::default(),
            discover: crate::config::discover::DiscoverConfig::default(),
            response_headers:
                crate::config::response_headers::ResponseHeadersConfig::default(
                ),
            observability:
                crate::config::observability::ObservabilityConfig::default(),
            deployment_target:
                crate::config::deployment_target::DeploymentTarget::default(),
            client_access:
                crate::config::client_access::ClientAccessConfig::default(),
            control_plane:
                crate::config::control_plane::ControlPlaneConfig::default(),
            default_model_mapping:
                crate::config::model_mapping::ModelMappingConfig::default(),
            helicone: crate::config::helicone::HeliconeConfig::default(),
            providers,
            provider_limits:
                crate::config::provider_limits::ProviderLimitCatalog::default(),
            credentials,
            cache_store: Some(crate::config::cache::CacheStore::default()),
            rate_limit_store: None,
            decision: crate::config::decision::DecisionEngineConfig::default(),
            global: MiddlewareConfig::default(),
            unified_api: MiddlewareConfig::default(),
            routers: crate::config::router::RouterConfigs::default(),
        }
    }
}

impl Config {
    #[must_use]
    pub fn autodefault_router_id() -> crate::types::router::RouterId {
        crate::types::router::RouterId::Named(CompactString::new("autodefault"))
    }

    #[must_use]
    pub fn has_autodefault_router(&self) -> bool {
        self.deployment_target.is_sidecar()
            && self.routers.contains_key(&Self::autodefault_router_id())
    }

    #[must_use]
    pub fn has_decision_enabled_router(&self) -> bool {
        self.routers.values().any(|router| router.decision.enabled)
    }

    /// `reqwest` gzip response decompression: per-provider override, else
    /// dispatcher default.
    #[must_use]
    pub fn gzip_decompress_responses_for(
        &self,
        provider: &InferenceProvider,
    ) -> bool {
        self.providers
            .get(provider)
            .and_then(|cfg| cfg.gzip_decompress_responses)
            .unwrap_or(self.dispatcher.gzip_decompress_responses)
    }
}

#[cfg(test)]
mod tests {
    use super::Config;
    use crate::types::provider::InferenceProvider;

    #[test]
    fn gzip_for_provider_uses_dispatcher_when_no_override() {
        let cfg = Config::default();
        assert!(cfg.dispatcher.gzip_decompress_responses);
        assert!(cfg.gzip_decompress_responses_for(&InferenceProvider::OpenAI));
    }
}
