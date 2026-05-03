use std::{collections::HashMap, fmt, sync::Mutex};

use r2d2::Pool;
use redis::Client;
use uuid::Uuid;

#[async_trait::async_trait]
pub trait StateStore: Send + Sync + fmt::Debug {
    async fn reserve(&self, key: &str, amount: i64) -> Result<String, String>;

    async fn commit_reservation(
        &self,
        key: &str,
        reservation_id: &str,
        final_amount: i64,
    ) -> Result<(), String>;

    async fn refund_reservation(
        &self,
        key: &str,
        reservation_id: &str,
    ) -> Result<(), String>;
}

#[derive(Debug)]
pub struct MemoryStateStore {
    state: Mutex<MemoryState>,
}

#[derive(Debug)]
struct MemoryState {
    usage: HashMap<String, i64>,
    reservations: HashMap<String, (String, i64)>,
}

impl MemoryStateStore {
    #[must_use]
    pub fn new() -> Self {
        Self {
            state: Mutex::new(MemoryState {
                usage: HashMap::new(),
                reservations: HashMap::new(),
            }),
        }
    }
}

impl Default for MemoryStateStore {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl StateStore for MemoryStateStore {
    async fn reserve(&self, key: &str, amount: i64) -> Result<String, String> {
        let reservation_id = Uuid::new_v4().to_string();
        let mut state = self.state.lock().map_err(|_| "state lock poisoned")?;

        state
            .reservations
            .insert(reservation_id.clone(), (key.to_string(), amount));
        *state.usage.entry(key.to_string()).or_insert(0) += amount;

        Ok(reservation_id)
    }

    async fn commit_reservation(
        &self,
        key: &str,
        reservation_id: &str,
        final_amount: i64,
    ) -> Result<(), String> {
        let mut state = self.state.lock().map_err(|_| "state lock poisoned")?;
        let Some((stored_key, reserved_amount)) =
            state.reservations.remove(reservation_id)
        else {
            return Err("reservation not found".to_string());
        };
        if stored_key != key {
            return Err("reservation key mismatch".to_string());
        }

        *state.usage.entry(key.to_string()).or_insert(0) +=
            final_amount - reserved_amount;
        Ok(())
    }

    async fn refund_reservation(
        &self,
        key: &str,
        reservation_id: &str,
    ) -> Result<(), String> {
        self.commit_reservation(key, reservation_id, 0).await
    }
}

pub struct RedisStateStore {
    pool: Pool<Client>,
}

impl fmt::Debug for RedisStateStore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RedisStateStore").finish_non_exhaustive()
    }
}

impl RedisStateStore {
    #[must_use]
    pub fn new(pool: Pool<Client>) -> Self {
        Self { pool }
    }
}

#[async_trait::async_trait]
impl StateStore for RedisStateStore {
    async fn reserve(&self, key: &str, amount: i64) -> Result<String, String> {
        let mut conn =
            self.pool.get().map_err(|e| format!("pool error: {e}"))?;
        let reservation_id = Uuid::new_v4().to_string();
        let usage_key = format!("usage:{key}");
        let reservation_key = format!("res:{reservation_id}");

        redis::pipe()
            .atomic()
            .incr(&usage_key, amount)
            .hset(&reservation_key, "amount", amount)
            .hset(&reservation_key, "key", key)
            .query::<()>(&mut conn)
            .map_err(|e| format!("redis error: {e}"))?;

        Ok(reservation_id)
    }

    async fn commit_reservation(
        &self,
        key: &str,
        reservation_id: &str,
        final_amount: i64,
    ) -> Result<(), String> {
        let mut conn =
            self.pool.get().map_err(|e| format!("pool error: {e}"))?;
        let reservation_key = format!("res:{reservation_id}");

        let (stored_key, reserved_amount): (Option<String>, Option<i64>) =
            redis::pipe()
                .hget(&reservation_key, "key")
                .hget(&reservation_key, "amount")
                .query(&mut conn)
                .map_err(|e| format!("redis error: {e}"))?;

        let (stored_key, reserved_amount) = stored_key
            .zip(reserved_amount)
            .ok_or_else(|| "reservation not found".to_string())?;
        if stored_key != key {
            return Err("reservation key mismatch".to_string());
        }

        let usage_key = format!("usage:{key}");
        redis::pipe()
            .atomic()
            .incr(usage_key, final_amount - reserved_amount)
            .del(&reservation_key)
            .query::<()>(&mut conn)
            .map_err(|e| format!("redis error: {e}"))?;

        Ok(())
    }

    async fn refund_reservation(
        &self,
        key: &str,
        reservation_id: &str,
    ) -> Result<(), String> {
        self.commit_reservation(key, reservation_id, 0).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    impl MemoryStateStore {
        fn usage(&self, key: &str) -> i64 {
            *self
                .state
                .lock()
                .expect("state lock")
                .usage
                .get(key)
                .unwrap_or(&0)
        }
    }

    #[tokio::test]
    async fn memory_store_commits_final_amount() {
        let store = MemoryStateStore::new();
        let reservation = store.reserve("user", 100).await.unwrap();

        store
            .commit_reservation("user", &reservation, 40)
            .await
            .unwrap();

        assert_eq!(store.usage("user"), 40);
    }

    #[tokio::test]
    async fn memory_store_refunds_reserved_amount() {
        let store = MemoryStateStore::new();
        let reservation = store.reserve("user", 100).await.unwrap();

        store
            .refund_reservation("user", &reservation)
            .await
            .unwrap();

        assert_eq!(store.usage("user"), 0);
    }
}
