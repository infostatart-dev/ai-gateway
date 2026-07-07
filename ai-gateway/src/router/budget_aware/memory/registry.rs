use std::time::Duration;

use compact_str::CompactString;
use moka::future::Cache;

use super::binding::{RouteBinding, RouteBindingPreference, RouteMemoryKey};

const TTL: Duration = Duration::from_mins(30);
const CAPACITY: u64 = 10_000;
const MAX_PREFERENCES: usize = 3;

#[derive(Debug)]
pub struct GatewayRouteMemory {
    cache: Cache<CompactString, Vec<RouteBindingEntry>>,
}

#[derive(Debug, Clone)]
struct RouteBindingEntry {
    binding: RouteBinding,
    score: f64,
    successes: u32,
    failures: u32,
    last_seen: u64,
}

impl Default for GatewayRouteMemory {
    fn default() -> Self {
        Self::new()
    }
}

impl GatewayRouteMemory {
    #[must_use]
    pub fn new() -> Self {
        Self {
            cache: Cache::builder()
                .max_capacity(CAPACITY)
                .time_to_live(TTL)
                .build(),
        }
    }

    pub async fn get(&self, key: &RouteMemoryKey) -> Option<RouteBinding> {
        let key = CompactString::from(key.as_str());
        self.cache.get(&key).await.and_then(|entries| {
            entries.first().map(|entry| entry.binding.clone())
        })
    }

    pub async fn preferred(
        &self,
        key: &RouteMemoryKey,
    ) -> Vec<RouteBindingPreference> {
        let key = CompactString::from(key.as_str());
        self.cache
            .get(&key)
            .await
            .unwrap_or_default()
            .into_iter()
            .map(|entry| RouteBindingPreference {
                binding: entry.binding,
                score: entry.score,
            })
            .collect()
    }

    pub async fn record(&self, key: &RouteMemoryKey, binding: RouteBinding) {
        self.record_weighted(key, binding, 1.0, 1.0, 10.0).await;
    }

    pub async fn record_degraded(
        &self,
        key: &RouteMemoryKey,
        binding: RouteBinding,
    ) {
        self.record_weighted(key, binding, 0.25, 0.25, 0.25).await;
    }

    async fn record_weighted(
        &self,
        key: &RouteMemoryKey,
        binding: RouteBinding,
        score_delta: f64,
        initial_score: f64,
        score_cap: f64,
    ) {
        let key = CompactString::from(key.as_str());
        let mut entries = self.cache.get(&key).await.unwrap_or_default();
        let next_seen = next_last_seen(&entries);
        if let Some(entry) = entries.iter_mut().find(|entry| {
            entry.binding.credential_id == binding.credential_id
                && entry.binding.model == binding.model
        }) {
            entry.successes = entry.successes.saturating_add(1);
            entry.score = (entry.score + score_delta).min(score_cap);
            entry.last_seen = next_seen;
        } else {
            entries.push(RouteBindingEntry {
                binding,
                score: initial_score,
                successes: 1,
                failures: 0,
                last_seen: next_seen,
            });
        }
        sort_and_trim(&mut entries);
        self.cache.insert(key, entries).await;
    }

    pub async fn penalize(
        &self,
        key: &RouteMemoryKey,
        binding: &RouteBinding,
    ) -> bool {
        let cache_key = CompactString::from(key.as_str());
        let Some(mut entries) = self.cache.get(&cache_key).await else {
            return false;
        };
        let Some(index) = entries.iter().position(|entry| {
            entry.binding.credential_id == binding.credential_id
                && entry.binding.model == binding.model
        }) else {
            return false;
        };
        let next_seen = next_last_seen(&entries);
        let entry = &mut entries[index];
        entry.failures = entry.failures.saturating_add(1);
        entry.score -= 2.0;
        entry.last_seen = next_seen;
        if entry.score <= 0.0 {
            entries.remove(index);
        } else {
            sort_and_trim(&mut entries);
        }
        if entries.is_empty() {
            self.cache.invalidate(&cache_key).await;
        } else {
            self.cache.insert(cache_key, entries).await;
        }
        true
    }

    pub async fn invalidate(&self, key: &RouteMemoryKey) {
        let key = CompactString::from(key.as_str());
        self.cache.invalidate(&key).await;
    }
}

fn next_last_seen(entries: &[RouteBindingEntry]) -> u64 {
    entries
        .iter()
        .map(|entry| entry.last_seen)
        .max()
        .unwrap_or_default()
        .saturating_add(1)
}

fn sort_and_trim(entries: &mut Vec<RouteBindingEntry>) {
    entries.sort_by(|left, right| {
        right
            .score
            .partial_cmp(&left.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| right.successes.cmp(&left.successes))
            .then_with(|| left.failures.cmp(&right.failures))
            .then_with(|| right.last_seen.cmp(&left.last_seen))
    });
    entries.truncate(MAX_PREFERENCES);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::credentials::ProviderCredentialId;

    #[tokio::test]
    async fn record_and_reuse_binding() {
        let memory = GatewayRouteMemory::new();
        let key =
            RouteMemoryKey::new("intent=fast|json_schema=false|context=small");
        let binding = RouteBinding {
            credential_id: ProviderCredentialId::new("gemini-free-1"),
            model: "gemini-2.5-flash-lite".to_string(),
        };
        memory.record(&key, binding.clone()).await;
        let got = memory.get(&key).await;
        assert_eq!(got, Some(binding));
    }

    #[tokio::test]
    async fn remembers_top_n_bindings_not_single_binding() {
        let memory = GatewayRouteMemory::new();
        let key =
            RouteMemoryKey::new("intent=fast|json_schema=false|context=small");
        for index in 1..=4 {
            memory
                .record(
                    &key,
                    RouteBinding {
                        credential_id: ProviderCredentialId::new(format!(
                            "gemini-free-{index}"
                        )),
                        model: "gemini-2.5-flash-lite".to_string(),
                    },
                )
                .await;
        }

        let preferred = memory.preferred(&key).await;

        assert_eq!(preferred.len(), MAX_PREFERENCES);
        assert_eq!(
            preferred[0].binding.credential_id.as_str(),
            "gemini-free-4"
        );
        assert!(preferred.iter().any(|preference| {
            preference.binding.credential_id.as_str() == "gemini-free-2"
        }));
    }

    #[tokio::test]
    async fn degraded_binding_keeps_degraded_preference_score() {
        let memory = GatewayRouteMemory::new();
        let key =
            RouteMemoryKey::new("intent=fast|json_schema=true|context=small");
        let binding = RouteBinding {
            credential_id: ProviderCredentialId::new("llm7-default"),
            model: "gpt-oss:20b".to_string(),
        };

        memory.record_degraded(&key, binding.clone()).await;
        let preferred = memory.preferred(&key).await;

        assert_eq!(
            preferred,
            vec![RouteBindingPreference {
                binding,
                score: 0.25,
            }]
        );
    }

    #[tokio::test]
    async fn repeated_degraded_binding_does_not_become_full_preference() {
        let memory = GatewayRouteMemory::new();
        let key =
            RouteMemoryKey::new("intent=fast|json_schema=true|context=small");
        let binding = RouteBinding {
            credential_id: ProviderCredentialId::new("llm7-default"),
            model: "gpt-oss:20b".to_string(),
        };

        for _ in 0..8 {
            memory.record_degraded(&key, binding.clone()).await;
        }
        let preferred = memory.preferred(&key).await;

        assert_eq!(
            preferred,
            vec![RouteBindingPreference {
                binding,
                score: 0.25,
            }]
        );
    }

    #[tokio::test]
    async fn penalize_removes_only_failed_binding() {
        let memory = GatewayRouteMemory::new();
        let key =
            RouteMemoryKey::new("intent=fast|json_schema=false|context=small");
        let failed = RouteBinding {
            credential_id: ProviderCredentialId::new("llm7-default"),
            model: "gpt-oss:20b".to_string(),
        };
        let sibling = RouteBinding {
            credential_id: ProviderCredentialId::new("llm7-default"),
            model: "fast".to_string(),
        };
        memory.record(&key, failed.clone()).await;
        memory.record(&key, sibling.clone()).await;

        assert!(memory.penalize(&key, &failed).await);
        let preferred = memory.preferred(&key).await;

        assert_eq!(
            preferred,
            vec![RouteBindingPreference {
                binding: sibling,
                score: 1.0,
            }]
        );
    }

    #[tokio::test]
    async fn invalidate_clears_binding() {
        let memory = GatewayRouteMemory::new();
        let key =
            RouteMemoryKey::new("intent=fast|json_schema=false|context=small");
        let binding = RouteBinding {
            credential_id: ProviderCredentialId::new("gemini-free-2"),
            model: "gemini-2.5-flash".to_string(),
        };
        memory.record(&key, binding).await;
        memory.invalidate(&key).await;
        assert!(memory.get(&key).await.is_none());
    }
}
