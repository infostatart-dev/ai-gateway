use std::sync::Arc;

use opentelemetry::global;
use rustc_hash::FxHashMap as HashMap;
use tokio::sync::RwLock;

use super::cache::setup_cache;
use crate::{
    app_state::{AppState, InnerAppState},
    config::Config,
    control_plane::control_plane_state::StateWithMetadata,
    discover::monitor::{
        health::provider::HealthMonitorMap, metrics::EndpointMetricsRegistry,
        rate_limit::RateLimitMonitorMap,
    },
    error::init::InitError,
    logger::service::JawnClient,
    metrics::Metrics,
    store::{connect, minio::BaseMinioClient, router::RouterStore},
    types::provider::ProviderKeys,
};

pub async fn build_app_state(config: Config) -> Result<AppState, InitError> {
    let minio = BaseMinioClient::new(config.minio.clone())?;
    let router_store = if config.deployment_target.is_cloud() {
        let pg_pool = connect(&config.database).await?;
        let router_store = RouterStore::new(pg_pool.clone())?;
        Some(router_store)
    } else {
        None
    };
    let jawn_http_client = JawnClient::new()?;

    let meter = global::meter("ai-gateway");
    let metrics = Metrics::new(&meter);
    let endpoint_metrics = EndpointMetricsRegistry::new(&config);
    let health_monitor = HealthMonitorMap::default();
    let rate_limit_monitor = RateLimitMonitorMap::default();

    let global_rate_limit = config
        .global
        .rate_limit
        .as_ref()
        .map(|rl| {
            crate::config::rate_limit::limiter_config(&rl.limits).map(Arc::new)
        })
        .transpose()?;

    let cache_manager = setup_cache(&config, metrics.clone());

    let helicone_api_keys =
        super::helicone_init::load_initial_helicone_api_keys(
            &config,
            router_store.as_ref(),
            &metrics,
        )
        .await?;

    let provider_keys = ProviderKeys::new(&config, &metrics);

    let decision_state = super::decision_state::build_decision_state(&config)?;

    Ok(AppState(Arc::new(InnerAppState {
        config,
        minio,
        router_store,
        jawn_http_client,
        control_plane_state: Arc::new(
            RwLock::new(StateWithMetadata::default()),
        ),
        provider_keys,
        global_rate_limit,
        router_rate_limits: RwLock::new(HashMap::default()),
        metrics,
        endpoint_metrics,
        health_monitors: health_monitor,
        rate_limit_monitors: rate_limit_monitor,
        rate_limit_senders: RwLock::new(HashMap::default()),
        rate_limit_receivers: RwLock::new(HashMap::default()),
        cache_manager,
        router_tx: RwLock::new(None),
        helicone_api_keys: RwLock::new(helicone_api_keys),
        router_organization_map: RwLock::new(HashMap::default()),
        traffic_shaper: decision_state.traffic_shaper,
        state_store: decision_state.state_store,
        policy_store: decision_state.policy_store,
    })))
}
