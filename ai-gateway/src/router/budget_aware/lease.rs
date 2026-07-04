use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use super::types::{BudgetAwareRouter, BudgetCandidate};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RouteLeaseKey {
    provider: String,
    scope: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RouteLeaseSnapshot {
    pub active: u32,
    pub limit: u32,
}

#[derive(Debug, Default)]
pub struct InFlightRouteRegistry {
    inner: Arc<Mutex<HashMap<RouteLeaseKey, u32>>>,
}

#[derive(Debug)]
pub struct RouteLease {
    inner: Arc<Mutex<HashMap<RouteLeaseKey, u32>>>,
    key: RouteLeaseKey,
}

pub(super) type RouteLeaseAdmission =
    Result<Option<RouteLease>, RouteLeaseSnapshot>;

impl Drop for RouteLease {
    fn drop(&mut self) {
        let mut entries = self.inner.lock().expect("route lease registry");
        let Some(active) = entries.get_mut(&self.key) else {
            return;
        };
        *active = active.saturating_sub(1);
        if *active == 0 {
            entries.remove(&self.key);
        }
    }
}

impl InFlightRouteRegistry {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn snapshot(
        &self,
        key: &RouteLeaseKey,
        limit: u32,
    ) -> RouteLeaseSnapshot {
        let entries = self.inner.lock().expect("route lease registry");
        RouteLeaseSnapshot {
            active: entries.get(key).copied().unwrap_or(0),
            limit,
        }
    }

    pub fn try_acquire(
        &self,
        key: RouteLeaseKey,
        limit: u32,
    ) -> Option<RouteLease> {
        let mut entries = self.inner.lock().expect("route lease registry");
        let active = entries.get(&key).copied().unwrap_or(0);
        if active >= limit {
            return None;
        }
        entries.insert(key.clone(), active.saturating_add(1));
        Some(RouteLease {
            inner: Arc::clone(&self.inner),
            key,
        })
    }

    pub fn acquire_unchecked(&self, key: RouteLeaseKey) -> RouteLease {
        let mut entries = self.inner.lock().expect("route lease registry");
        let active = entries.get(&key).copied().unwrap_or(0);
        entries.insert(key.clone(), active.saturating_add(1));
        RouteLease {
            inner: Arc::clone(&self.inner),
            key,
        }
    }
}

pub(super) fn route_lease_target(
    router: &BudgetAwareRouter,
    candidate: &BudgetCandidate,
) -> Option<(RouteLeaseKey, u32)> {
    let model = candidate.capability.model.to_string();
    let pacing = router.app_state.upstream_pacing();
    let scope = pacing.scope_key_for(
        &candidate.capability.provider,
        Some(&candidate.credential_id),
        Some(candidate.credential_tier.as_str()),
        Some(model.as_str()),
    )?;
    let limits = pacing.limits_for_candidate(
        &candidate.capability.provider,
        Some(candidate.credential_tier.as_str()),
        Some(model.as_str()),
    )?;
    Some((
        RouteLeaseKey {
            provider: candidate.capability.provider.to_string(),
            scope,
        },
        u32::try_from(limits.concurrent).unwrap_or(u32::MAX).max(1),
    ))
}

impl BudgetAwareRouter {
    #[must_use]
    pub(super) fn route_lease_snapshot(
        &self,
        candidate: &BudgetCandidate,
    ) -> Option<RouteLeaseSnapshot> {
        let (key, limit) = route_lease_target(self, candidate)?;
        Some(self.app_state.route_leases().snapshot(&key, limit))
    }

    pub(super) fn try_acquire_route_lease(
        &self,
        candidate: &BudgetCandidate,
        has_next_candidate: bool,
    ) -> RouteLeaseAdmission {
        let Some((key, limit)) = route_lease_target(self, candidate) else {
            return Ok(None);
        };
        if has_next_candidate {
            return self
                .app_state
                .route_leases()
                .try_acquire(key.clone(), limit)
                .map(Some)
                .ok_or_else(|| {
                    self.app_state.route_leases().snapshot(&key, limit)
                });
        }
        Ok(Some(self.app_state.route_leases().acquire_unchecked(key)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key() -> RouteLeaseKey {
        RouteLeaseKey {
            provider: "vllm".into(),
            scope: "credential:vllm-anonymous".into(),
        }
    }

    #[test]
    fn try_acquire_respects_limit_and_releases_on_drop() {
        let registry = InFlightRouteRegistry::new();
        let key = key();

        let lease = registry.try_acquire(key.clone(), 1).expect("first lease");
        assert_eq!(
            registry.snapshot(&key, 1),
            RouteLeaseSnapshot {
                active: 1,
                limit: 1
            }
        );
        assert!(registry.try_acquire(key.clone(), 1).is_none());

        drop(lease);
        assert_eq!(
            registry.snapshot(&key, 1),
            RouteLeaseSnapshot {
                active: 0,
                limit: 1
            }
        );
        assert!(registry.try_acquire(key, 1).is_some());
    }
}
