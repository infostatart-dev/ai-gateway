use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;

/// Defines the contract for an atomic state store for budget usage.
#[async_trait::async_trait]
pub trait StateStore: Send + Sync {
    /// Atomically increment the usage counter. Returns the new total.
    async fn increment_usage(&self, key: &str, amount: f64) -> Result<f64, String>;
    
    /// Reserve budget. Creates a lease.
    async fn reserve(&self, key: &str, amount: f64, ttl_secs: u64) -> Result<String, String>;
    
    /// Commit a reservation (finalize the actual usage, which might be less than reserved).
    async fn commit_reservation(&self, key: &str, reservation_id: &str, final_amount: f64) -> Result<(), String>;
    
    /// Refund/cancel a reservation entirely.
    async fn refund_reservation(&self, key: &str, reservation_id: &str) -> Result<(), String>;
    
    /// Renew a lease for a given reservation ID.
    async fn renew_lease(&self, key: &str, reservation_id: &str, ttl_secs: u64) -> Result<(), String>;
    
    /// Evict expired leases and return the refunded budget for tracking
    async fn reconcile_expired_leases(&self) -> Result<Vec<(String, f64)>, String>;
}

/// A simple in-memory implementation of the StateStore for fallback or free-tier usage without Redis.
pub struct MemoryStateStore {
    // Basic in-memory counters (e.g. key -> used amount)
    usage: Mutex<std::collections::HashMap<String, f64>>,
    // Active reservations: reservation_id -> (key, amount, expires_at)
    reservations: Mutex<std::collections::HashMap<String, (String, f64, std::time::Instant)>>,
}

impl MemoryStateStore {
    pub fn new() -> Self {
        Self {
            usage: Mutex::new(std::collections::HashMap::new()),
            reservations: Mutex::new(std::collections::HashMap::new()),
        }
    }
}

#[async_trait::async_trait]
impl StateStore for MemoryStateStore {
    async fn increment_usage(&self, key: &str, amount: f64) -> Result<f64, String> {
        let mut usage = self.usage.lock().await;
        let current = usage.entry(key.to_string()).or_insert(0.0);
        *current += amount;
        Ok(*current)
    }

    async fn reserve(&self, key: &str, amount: f64, ttl_secs: u64) -> Result<String, String> {
        let reservation_id = Uuid::new_v4().to_string();
        let expires_at = std::time::Instant::now() + std::time::Duration::from_secs(ttl_secs);
        let mut reservations = self.reservations.lock().await;
        reservations.insert(reservation_id.clone(), (key.to_string(), amount, expires_at));
        
        self.increment_usage(key, amount).await?;
        
        Ok(reservation_id)
    }

    async fn commit_reservation(&self, key: &str, reservation_id: &str, final_amount: f64) -> Result<(), String> {
        let mut reservations = self.reservations.lock().await;
        if let Some((stored_key, reserved_amount, _)) = reservations.remove(reservation_id) {
            if stored_key == key {
                let diff = final_amount - reserved_amount;
                // Temporarily release lock to avoid deadlock if increment_usage tried to acquire it,
                // but increment_usage acquires usage, not reservations. So we're fine, but since we call `&self`
                // we should drop the lock to be safe.
                drop(reservations);
                self.increment_usage(key, diff).await?;
                return Ok(());
            }
        }
        Err("Reservation not found".to_string())
    }

    async fn refund_reservation(&self, key: &str, reservation_id: &str) -> Result<(), String> {
        self.commit_reservation(key, reservation_id, 0.0).await
    }

    async fn renew_lease(&self, _key: &str, reservation_id: &str, ttl_secs: u64) -> Result<(), String> {
        let mut reservations = self.reservations.lock().await;
        if let Some((_, _, expires_at)) = reservations.get_mut(reservation_id) {
            *expires_at = std::time::Instant::now() + std::time::Duration::from_secs(ttl_secs);
            Ok(())
        } else {
            Err("Reservation not found".to_string())
        }
    }

    async fn reconcile_expired_leases(&self) -> Result<Vec<(String, f64)>, String> {
        let mut reservations = self.reservations.lock().await;
        let now = std::time::Instant::now();
        let mut expired = Vec::new();
        
        // Retain only active leases, collect expired ones
        reservations.retain(|reservation_id, (key, amount, expires_at)| {
            if *expires_at < now {
                expired.push((reservation_id.clone(), key.clone(), *amount));
                false
            } else {
                true
            }
        });
        
        drop(reservations);
        let mut refunds = Vec::new();
        
        for (_res_id, key, amount) in expired {
            self.increment_usage(&key, -amount).await?;
            refunds.push((key, amount));
        }
        
        Ok(refunds)
    }
}

pub struct BudgetManager {
    store: Arc<dyn StateStore>,
}

impl BudgetManager {
    pub fn new(store: Arc<dyn StateStore>) -> Self {
        Self { store }
    }
    
    /// Spawns a background task that periodically reconciles expired leases.
    pub fn spawn_reconciliation_task(store: Arc<dyn StateStore>, interval_secs: u64) {
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(interval_secs));
            loop {
                interval.tick().await;
                match store.reconcile_expired_leases().await {
                    Ok(refunds) => {
                        if !refunds.is_empty() {
                            tracing::info!(count = refunds.len(), "reconciled expired budget leases");
                        }
                    }
                    Err(e) => {
                        tracing::error!(error = %e, "failed to reconcile expired budget leases");
                    }
                }
            }
        });
    }
}
