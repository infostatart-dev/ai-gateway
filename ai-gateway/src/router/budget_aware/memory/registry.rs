use std::time::Duration;

use compact_str::CompactString;
use moka::future::Cache;

use super::binding::RouteBinding;

const TTL: Duration = Duration::from_mins(30);
const CAPACITY: u64 = 10_000;

#[derive(Debug)]
pub struct WorkUnitRouteMemory {
    cache: Cache<(CompactString, CompactString), RouteBinding>,
}

impl Default for WorkUnitRouteMemory {
    fn default() -> Self {
        Self::new()
    }
}

impl WorkUnitRouteMemory {
    #[must_use]
    pub fn new() -> Self {
        Self {
            cache: Cache::builder()
                .max_capacity(CAPACITY)
                .time_to_live(TTL)
                .build(),
        }
    }

    pub async fn get(
        &self,
        agent_name: &str,
        work_unit_id: &str,
    ) -> Option<RouteBinding> {
        let key = (
            CompactString::from(agent_name),
            CompactString::from(work_unit_id),
        );
        self.cache.get(&key).await
    }

    pub async fn record(
        &self,
        agent_name: &str,
        work_unit_id: &str,
        binding: RouteBinding,
    ) {
        let key = (
            CompactString::from(agent_name),
            CompactString::from(work_unit_id),
        );
        self.cache.insert(key, binding).await;
    }

    pub async fn invalidate(&self, agent_name: &str, work_unit_id: &str) {
        let key = (
            CompactString::from(agent_name),
            CompactString::from(work_unit_id),
        );
        self.cache.invalidate(&key).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::credentials::ProviderCredentialId;

    #[tokio::test]
    async fn record_and_reuse_binding() {
        let memory = WorkUnitRouteMemory::new();
        let binding = RouteBinding {
            credential_id: ProviderCredentialId::new("gemini-free-1"),
            model: "gemini-2.5-flash-lite".to_string(),
        };
        memory.record("agent-a", "unit-1", binding.clone()).await;
        let got = memory.get("agent-a", "unit-1").await;
        assert_eq!(got, Some(binding));
    }

    #[tokio::test]
    async fn invalidate_clears_binding() {
        let memory = WorkUnitRouteMemory::new();
        let binding = RouteBinding {
            credential_id: ProviderCredentialId::new("gemini-free-2"),
            model: "gemini-2.5-flash".to_string(),
        };
        memory.record("agent-b", "unit-2", binding).await;
        memory.invalidate("agent-b", "unit-2").await;
        assert!(memory.get("agent-b", "unit-2").await.is_none());
    }
}
