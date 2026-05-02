use std::sync::Arc;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

/// Represents a hierarchical traffic shaper using atomic semaphores.
pub struct TrafficShaper {
    global_limit: Arc<Semaphore>,
    free_tier_limit: Arc<Semaphore>,
    paid_tier_limit: Arc<Semaphore>,
    // In a real system, provider limits might be mapped dynamically.
    // For demonstration, we'll store a generic provider limit.
    provider_limit: Arc<Semaphore>,
}

impl TrafficShaper {
    pub fn new(global: usize, free_tier: usize, paid_tier: usize, provider: usize) -> Self {
        Self {
            global_limit: Arc::new(Semaphore::new(global)),
            free_tier_limit: Arc::new(Semaphore::new(free_tier)),
            paid_tier_limit: Arc::new(Semaphore::new(paid_tier)),
            provider_limit: Arc::new(Semaphore::new(provider)),
        }
    }

    /// Acquire a slot across all relevant hierarchies.
    /// Returns a combined permit that keeps all semaphores locked until dropped.
    pub async fn acquire(
        &self,
        is_free_tier: bool,
        timeout: std::time::Duration,
    ) -> Result<CombinedPermit, String> {
        // Try acquiring all needed permits concurrently or sequentially with timeout.
        // We do it sequentially with the overarching timeout to avoid deadlock and ensure order.
        let acquire_future = async {
            // 1. Global
            let global_permit = self.global_limit.clone().acquire_owned().await.map_err(|_| "Closed")?;
            
            // 2. Tier
            let tier_permit = if is_free_tier {
                self.free_tier_limit.clone().acquire_owned().await.map_err(|_| "Closed")?
            } else {
                self.paid_tier_limit.clone().acquire_owned().await.map_err(|_| "Closed")?
            };
            
            // 3. Provider
            let provider_permit = self.provider_limit.clone().acquire_owned().await.map_err(|_| "Closed")?;

            Ok(CombinedPermit {
                _global: global_permit,
                _tier: tier_permit,
                _provider: provider_permit,
            })
        };

        match tokio::time::timeout(timeout, acquire_future).await {
            Ok(result) => result,
            Err(_) => Err("Timeout acquiring traffic slot".to_string()),
        }
    }
}

pub struct CombinedPermit {
    _global: OwnedSemaphorePermit,
    _tier: OwnedSemaphorePermit,
    _provider: OwnedSemaphorePermit,
}
