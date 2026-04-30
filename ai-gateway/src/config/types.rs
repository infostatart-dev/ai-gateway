use serde::{Deserialize, Serialize};
use thiserror::Error;
use displaydoc::Display;
use config::ConfigError;

pub const ROUTER_ID_REGEX: &str = r"^[A-Za-z0-9_-]{1,12}$";
pub const DEFAULT_CONFIG_PATH: &str = "/etc/ai-gateway/config.yaml";

#[derive(Debug, Error, Display)]
pub enum Error {
    /// error collecting config sources: {0}
    Source(#[from] ConfigError),
    /// deserialization error for input config: {0}
    InputConfigDeserialization(#[from] serde_path_to_error::Error<ConfigError>),
    /// deserialization error for merged config: {0}
    MergedConfigDeserialization(#[from] serde_path_to_error::Error<serde_json::Error>),
    /// URL parsing error: {0}
    UrlParse(#[from] url::ParseError),
}

#[derive(Debug, Default, Deserialize, Serialize, PartialEq, Eq, Hash)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct MiddlewareConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache: Option<crate::config::cache::CacheConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rate_limit: Option<crate::config::rate_limit::RateLimitConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retries: Option<crate::config::retry::RetryConfig>,
}

#[derive(Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields, rename_all = "kebab-case")]
pub struct Config {
    pub telemetry: telemetry::Config,
    pub server: crate::config::server::ServerConfig,
    pub minio: crate::config::minio::Config,
    pub database: crate::config::database::DatabaseConfig,
    pub dispatcher: crate::config::dispatcher::DispatcherConfig,
    pub discover: crate::config::discover::DiscoverConfig,
    pub response_headers: crate::config::response_headers::ResponseHeadersConfig,
    pub deployment_target: crate::config::deployment_target::DeploymentTarget,
    pub control_plane: crate::config::control_plane::ControlPlaneConfig,
    pub default_model_mapping: crate::config::model_mapping::ModelMappingConfig,
    pub helicone: crate::config::helicone::HeliconeConfig,
    pub providers: crate::config::providers::ProvidersConfig,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_store: Option<crate::config::cache::CacheStore>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rate_limit_store: Option<crate::config::rate_limit::RateLimitStore>,
    pub global: MiddlewareConfig,
    pub unified_api: MiddlewareConfig,
    pub routers: crate::config::router::RouterConfigs,
}
