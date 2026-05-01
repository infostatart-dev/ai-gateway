use http_cache::MokaManager;
use moka::future::Cache;

use crate::{
    cache::{CacheClient, RedisCacheManager},
    config::{Config, cache::CacheStore},
    error::init::InitError,
    metrics::Metrics,
};

pub fn setup_cache(config: &Config, metrics: Metrics) -> Option<CacheClient> {
    match &config.cache_store {
        Some(CacheStore::InMemory { max_size }) => {
            tracing::debug!("Using in-memory cache");
            let moka_manager = setup_moka_cache(*max_size, metrics);
            Some(CacheClient::Moka(moka_manager))
        }
        Some(CacheStore::Redis { host_url }) => {
            tracing::debug!("Using redis cache");
            match setup_redis_cache(host_url.clone()) {
                Ok(redis_manager) => {
                    tracing::info!("Successfully connected to Redis cache");
                    Some(CacheClient::Redis(redis_manager))
                }
                Err(e) => {
                    tracing::error!(
                        "Failed to connect to Redis cache at {}: {}",
                        host_url,
                        e
                    );
                    None
                }
            }
        }
        None => None,
    }
}

fn setup_moka_cache(capacity: usize, metrics: Metrics) -> MokaManager {
    let listener = move |_k, _v, cause| {
        use moka::notification::RemovalCause;
        if cause == RemovalCause::Size {
            metrics.cache.evictions.add(1, &[]);
        }
    };

    let cache = Cache::builder()
        .max_capacity(u64::try_from(capacity).unwrap_or(u64::MAX))
        .eviction_listener(listener)
        .build();
    MokaManager::new(cache)
}

fn setup_redis_cache(
    host_url: url::Url,
) -> Result<RedisCacheManager, InitError> {
    RedisCacheManager::new(host_url)
}
