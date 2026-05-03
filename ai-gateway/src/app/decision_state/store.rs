use std::sync::Arc;

use crate::{config::Config, error::init::InitError};

pub(super) fn build_decision_state_store(
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
