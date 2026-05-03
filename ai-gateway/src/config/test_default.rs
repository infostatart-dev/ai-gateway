#[cfg(feature = "testing")]
use crate::{
    config::{Config, MiddlewareConfig},
    tests::TestDefault,
};

#[cfg(feature = "testing")]
impl TestDefault for Config {
    fn test_default() -> Self {
        let telemetry = telemetry::Config {
            exporter: telemetry::Exporter::Stdout,
            level: "info,ai_gateway=trace".to_string(),
            ..Default::default()
        };
        Config {
            telemetry,
            server: crate::config::server::ServerConfig::test_default(),
            minio: crate::config::minio::Config::test_default(),
            database: crate::config::database::DatabaseConfig::test_default(),
            dispatcher:
                crate::config::dispatcher::DispatcherConfig::test_default(),
            control_plane:
                crate::config::control_plane::ControlPlaneConfig::default(),
            default_model_mapping:
                crate::config::model_mapping::ModelMappingConfig::default(),
            global: MiddlewareConfig::default(),
            unified_api: MiddlewareConfig::default(),
            providers: crate::config::providers::ProvidersConfig::default(),
            provider_limits:
                crate::config::provider_limits::ProviderLimitCatalog::default(),
            helicone: crate::config::helicone::HeliconeConfig::test_default(),
            deployment_target:
                crate::config::deployment_target::DeploymentTarget::Sidecar,
            discover: crate::config::discover::DiscoverConfig::test_default(),
            cache_store: Some(crate::config::cache::CacheStore::default()),
            rate_limit_store: Some(
                crate::config::rate_limit::RateLimitStore::default(),
            ),
            decision: crate::config::decision::DecisionEngineConfig::default(),
            routers: crate::config::router::RouterConfigs::test_default(),
            response_headers:
                crate::config::response_headers::ResponseHeadersConfig::default(
                ),
        }
    }
}
