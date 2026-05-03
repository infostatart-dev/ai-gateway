use std::sync::Mutex;

use super::inner::MemoryState;
use crate::middleware::decision::budget::trait_def::StateStore;

#[derive(Debug)]
pub struct MemoryStateStore {
    state: Mutex<MemoryState>,
}

impl MemoryStateStore {
    #[must_use]
    pub fn new() -> Self {
        Self {
            state: Mutex::new(MemoryState::empty()),
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
        let mut state = self.state.lock().map_err(|_| "state lock poisoned")?;
        Ok(state.reserve(key, amount))
    }

    async fn commit_reservation(
        &self,
        key: &str,
        reservation_id: &str,
        final_amount: i64,
    ) -> Result<(), String> {
        let mut state = self.state.lock().map_err(|_| "state lock poisoned")?;
        state.apply_commit(key, reservation_id, final_amount)
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
impl MemoryStateStore {
    pub(crate) fn usage_snapshot(&self, key: &str) -> i64 {
        *self
            .state
            .lock()
            .expect("state lock")
            .usage
            .get(key)
            .unwrap_or(&0)
    }
}
