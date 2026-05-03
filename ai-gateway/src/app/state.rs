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

    let helicone_api_keys = if config.deployment_target.is_cloud()
        && let Some(router_store_ref) = router_store.as_ref()
    {
        let keys = router_store_ref
            .get_all_helicone_api_keys()
            .await
            .map_err(|e| InitError::InitHeliconeKeys(e.to_string()))?;
        tracing::info!("loaded initial {} helicone api keys", keys.len());
        metrics
            .routers
            .helicone_api_keys
            .add(i64::try_from(keys.len()).unwrap_or(i64::MAX), &[]);
        Some(keys)
    } else {
        None
    };

    let provider_keys = ProviderKeys::new(&config, &metrics);

    let decision_state = build_decision_state(&config)?;

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

struct DecisionState {
    traffic_shaper: Arc<crate::middleware::decision::shaping::TrafficShaper>,
    state_store: Arc<dyn crate::middleware::decision::budget::StateStore>,
    policy_store: Arc<dyn crate::middleware::decision::policy::PolicyStore>,
}

fn build_decision_state(config: &Config) -> Result<DecisionState, InitError> {
    validate_decision_config(config)?;

    Ok(DecisionState {
        traffic_shaper: Arc::new(
            crate::middleware::decision::shaping::TrafficShaper::new(
                config.decision.shaper.global,
                config.decision.shaper.free_tier,
                config.decision.shaper.paid_tier,
                config.decision.shaper.provider,
            ),
        ),
        state_store: build_decision_state_store(config)?,
        policy_store: Arc::new(
            crate::middleware::decision::policy::MemoryPolicyStore::new(
                config.decision.policy_store.cache_capacity,
                config.decision.policy_store.cache_ttl,
                config.decision.default_policy.clone(),
            ),
        ),
    })
}

fn validate_decision_config(config: &Config) -> Result<(), InitError> {
    if config.decision.enabled && config.decision.default_policy.is_none() {
        return Err(InitError::InvalidDecisionConfig(
            "default policy not configured",
        ));
    }
    if config
        .decision
        .default_policy
        .as_ref()
        .is_some_and(|policy| policy.max_output_tokens == 0)
    {
        return Err(InitError::InvalidDecisionConfig(
            "default policy max output tokens must be greater than zero",
        ));
    }
    Ok(())
}

fn build_decision_state_store(
    config: &Config,
) -> Result<Arc<dyn crate::middleware::decision::budget::StateStore>, InitError>
{
    match &config.decision.state_store {
        Some(crate::config::decision::StateStoreConfig::Redis(redis_cfg)) => {
            let client =
                redis::Client::open(redis_cfg.host_url.expose().clone())
                    .map_err(InitError::CreateRedisClient)?;
            let pool = r2d2::Pool::builder()
                .build(client)
                .map_err(InitError::CreateRedisPool)?;
            Ok(Arc::new(
                crate::middleware::decision::budget::RedisStateStore::new(pool),
            ))
        }
        Some(crate::config::decision::StateStoreConfig::Memory) | None => {
            if config.decision.enabled && config.deployment_target.is_cloud() {
                return Err(InitError::DistributedStateStoreRequired);
            }
            Ok(Arc::new(
                crate::middleware::decision::budget::MemoryStateStore::new(),
            ))
        }
    }
}
