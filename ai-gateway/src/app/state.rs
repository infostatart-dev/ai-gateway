use std::sync::{Arc, RwLock as StdRwLock};

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
    let client_access_snapshot = load_initial_client_access_snapshot(&config)?;
    let client_access_quota_store = if config.client_access.enabled {
        Some(crate::client_access::quota::build_quota_store(
            &config.client_access.quota_store,
        )?)
    } else {
        None
    };

    let decision_state = super::decision_state::build_decision_state(&config)?;
    let upstream_pacing = Arc::new(crate::router::pacing::PacingRegistry::new(
        config.provider_limits.clone(),
    ));
    let budget_probe =
        Arc::new(crate::router::budget_probe::BudgetProbeRegistry::new(
            config.provider_limits.clone(),
        ));
    let route_memory =
        Arc::new(crate::router::budget_aware::WorkUnitRouteMemory::new());
    let route_leases =
        Arc::new(crate::router::budget_aware::InFlightRouteRegistry::new());

    Ok(AppState(Arc::new(InnerAppState {
        config,
        minio,
        router_store,
        jawn_http_client,
        control_plane_state: Arc::new(
            RwLock::new(StateWithMetadata::default()),
        ),
        provider_keys,
        client_access_snapshot,
        client_access_quota_store,
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
        upstream_pacing,
        budget_probe,
        route_memory,
        route_leases,
    })))
}

fn load_initial_client_access_snapshot(
    config: &Config,
) -> Result<
    Option<Arc<StdRwLock<Arc<crate::client_access::ClientAccessSnapshot>>>>,
    InitError,
> {
    if !config.client_access.enabled {
        return Ok(None);
    }
    if config.deployment_target.is_cloud()
        && matches!(
            config.client_access.quota_store,
            crate::config::client_access::ClientAccessQuotaStoreConfig::Memory
        )
    {
        tracing::warn!(
            "client access memory quota store is process-local and not \
             suitable for cloud deployments",
        );
    }
    let path = config.client_access.file.as_ref().ok_or_else(|| {
        InitError::InvalidClientAccessConfig(
            "`client-access.file` is required when client access is enabled"
                .to_string(),
        )
    })?;
    let snapshot = crate::client_access::loader::load_snapshot_from_file(path)
        .map_err(|err| InitError::InvalidClientAccessConfig(err.to_string()))?;
    tracing::info!(
        path = %path.display(),
        keys = snapshot.len(),
        "loaded client access snapshot",
    );
    Ok(Some(Arc::new(StdRwLock::new(snapshot))))
}
