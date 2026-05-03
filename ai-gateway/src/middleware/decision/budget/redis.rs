use std::fmt;

use r2d2::Pool;
use redis::Client;

use super::{redis_cmds, trait_def::StateStore};

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
        redis_cmds::reserve_atomic(&self.pool, key, amount)
    }

    async fn commit_reservation(
        &self,
        key: &str,
        reservation_id: &str,
        final_amount: i64,
    ) -> Result<(), String> {
        redis_cmds::commit_delta(&self.pool, key, reservation_id, final_amount)
    }

    async fn refund_reservation(
        &self,
        key: &str,
        reservation_id: &str,
    ) -> Result<(), String> {
        self.commit_reservation(key, reservation_id, 0).await
    }
}
