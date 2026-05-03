use std::{sync::Arc, time::Duration};

use tokio::sync::{OwnedSemaphorePermit, Semaphore};

pub struct TrafficShaper {
    global_limit: Arc<Semaphore>,
    free_tier_limit: Arc<Semaphore>,
    paid_tier_limit: Arc<Semaphore>,
    provider_limit: Arc<Semaphore>,
}

impl TrafficShaper {
    #[must_use]
    pub fn new(
        global: usize,
        free_tier: usize,
        paid_tier: usize,
        provider: usize,
    ) -> Self {
        Self {
            global_limit: Arc::new(Semaphore::new(global)),
            free_tier_limit: Arc::new(Semaphore::new(free_tier)),
            paid_tier_limit: Arc::new(Semaphore::new(paid_tier)),
            provider_limit: Arc::new(Semaphore::new(provider)),
        }
    }

    pub async fn acquire(
        &self,
        is_free_tier: bool,
        timeout: Duration,
    ) -> Result<CombinedPermit, String> {
        tokio::time::timeout(timeout, self.acquire_inner(is_free_tier))
            .await
            .map_err(|_| "timeout acquiring traffic slot".to_string())?
    }

    async fn acquire_inner(
        &self,
        is_free_tier: bool,
    ) -> Result<CombinedPermit, String> {
        let global = self
            .global_limit
            .clone()
            .acquire_owned()
            .await
            .map_err(|_| "global shaper closed".to_string())?;
        let tier = if is_free_tier {
            self.free_tier_limit.clone().acquire_owned().await
        } else {
            self.paid_tier_limit.clone().acquire_owned().await
        }
        .map_err(|_| "tier shaper closed".to_string())?;
        let provider = self
            .provider_limit
            .clone()
            .acquire_owned()
            .await
            .map_err(|_| "provider shaper closed".to_string())?;

        Ok(CombinedPermit {
            _global: global,
            _tier: tier,
            _provider: provider,
        })
    }
}

pub struct CombinedPermit {
    _global: OwnedSemaphorePermit,
    _tier: OwnedSemaphorePermit,
    _provider: OwnedSemaphorePermit,
}
