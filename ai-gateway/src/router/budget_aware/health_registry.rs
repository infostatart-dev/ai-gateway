use std::{
    collections::HashMap,
    sync::Mutex,
    time::{Duration, Instant},
};

use crate::{
    config::credentials::ProviderCredentialId,
    metrics::provider::attempt::CallOutcome,
    router::retry_after::FailoverClass, types::provider::InferenceProvider,
};

#[derive(Debug, Clone, serde::Serialize, PartialEq)]
pub struct RoutingHealthSnapshot {
    pub circuit_open: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub open_until: Option<chrono::DateTime<chrono::Utc>>,
    pub success_rate: f64,
    pub planner_excluded: bool,
}

const WINDOW: Duration = Duration::from_mins(5);
const MIN_WINDOW_ATTEMPTS: u32 = 5;
const CIRCUIT_SUCCESS_THRESHOLD: f64 = 0.10;
const CIRCUIT_TTL: Duration = Duration::from_mins(15);
const AUTH_CIRCUIT_TTL: Duration = Duration::from_mins(5);

#[derive(Debug, Default, Clone, Copy)]
struct WindowCounts {
    attempts: u32,
    successes: u32,
}

#[derive(Debug)]
struct CredentialHealthEntry {
    window: WindowCounts,
    window_started: Instant,
    circuit_open_until: Option<Instant>,
    last_failover_class: Option<FailoverClass>,
}

#[derive(Debug)]
pub struct CredentialHealthRegistry {
    entries: Mutex<HashMap<(String, String), CredentialHealthEntry>>,
}

impl Default for CredentialHealthRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl CredentialHealthRegistry {
    #[must_use]
    pub fn new() -> Self {
        Self {
            entries: Mutex::new(HashMap::new()),
        }
    }

    pub fn record_attempt(
        &self,
        provider: &InferenceProvider,
        credential: &ProviderCredentialId,
        outcome: CallOutcome,
        status_code: u16,
    ) {
        let now = Instant::now();
        let mut entries = self.entries.lock().expect("health registry");
        let entry = entries
            .entry((provider.to_string(), credential.to_string()))
            .or_insert_with(|| CredentialHealthEntry {
                window: WindowCounts::default(),
                window_started: now,
                circuit_open_until: None,
                last_failover_class: None,
            });
        if now.duration_since(entry.window_started) > WINDOW {
            entry.window = WindowCounts::default();
            entry.window_started = now;
        }
        entry.window.attempts = entry.window.attempts.saturating_add(1);
        if matches!(
            outcome,
            CallOutcome::Success | CallOutcome::SuccessDegraded
        ) {
            entry.window.successes = entry.window.successes.saturating_add(1);
            entry.circuit_open_until = None;
        }
        if status_code == 401 {
            entry.circuit_open_until = Some(now + AUTH_CIRCUIT_TTL);
        } else if entry.window.attempts >= MIN_WINDOW_ATTEMPTS
            && entry.success_rate() < CIRCUIT_SUCCESS_THRESHOLD
        {
            entry.circuit_open_until = Some(now + CIRCUIT_TTL);
        }
    }

    pub fn record_failover_class(
        &self,
        provider: &InferenceProvider,
        credential: &ProviderCredentialId,
        class: FailoverClass,
        scope: crate::router::retry_after::ExhaustionScope,
    ) {
        let now = Instant::now();
        let mut entries = self.entries.lock().expect("health registry");
        let entry = entries
            .entry((provider.to_string(), credential.to_string()))
            .or_insert_with(|| CredentialHealthEntry {
                window: WindowCounts::default(),
                window_started: now,
                circuit_open_until: None,
                last_failover_class: None,
            });
        entry.last_failover_class = Some(class);
        if matches!(
            scope,
            crate::router::retry_after::ExhaustionScope::Slot
                | crate::router::retry_after::ExhaustionScope::Project
        ) && matches!(
            class,
            FailoverClass::QuotaExhausted | FailoverClass::CredentialRestricted
        ) {
            entry.circuit_open_until = Some(now + CIRCUIT_TTL);
        }
    }

    #[must_use]
    pub fn is_circuit_open(
        &self,
        provider: &InferenceProvider,
        credential: &ProviderCredentialId,
        now: Instant,
    ) -> bool {
        let entries = self.entries.lock().expect("health registry");
        entries
            .get(&(provider.to_string(), credential.to_string()))
            .and_then(|entry| entry.circuit_open_until)
            .is_some_and(|until| until > now)
    }

    #[must_use]
    pub fn success_rate(
        &self,
        provider: &InferenceProvider,
        credential: &ProviderCredentialId,
    ) -> f64 {
        let entries = self.entries.lock().expect("health registry");
        entries
            .get(&(provider.to_string(), credential.to_string()))
            .map_or(1.0, CredentialHealthEntry::success_rate)
    }

    #[must_use]
    pub fn circuit_open_until(
        &self,
        provider: &InferenceProvider,
        credential: &ProviderCredentialId,
        now: Instant,
    ) -> Option<Instant> {
        let entries = self.entries.lock().expect("health registry");
        entries
            .get(&(provider.to_string(), credential.to_string()))
            .and_then(|entry| entry.circuit_open_until)
            .filter(|until| *until > now)
    }

    #[must_use]
    pub fn routing_health_snapshot(
        &self,
        provider: &InferenceProvider,
        credential: &ProviderCredentialId,
        now: Instant,
    ) -> RoutingHealthSnapshot {
        let circuit_open = self.is_circuit_open(provider, credential, now);
        let success_rate = self.success_rate(provider, credential);
        let planner_excluded = circuit_open
            || self.credential_zero_success_dead(provider, credential, now);
        let open_until = self
            .circuit_open_until(provider, credential, now)
            .map(|until| {
                let remaining = until.saturating_duration_since(now);
                chrono::Utc::now()
                    + chrono::Duration::from_std(remaining)
                        .unwrap_or_else(|_| chrono::Duration::zero())
            });
        RoutingHealthSnapshot {
            circuit_open,
            open_until,
            success_rate,
            planner_excluded,
        }
    }

    #[must_use]
    pub fn provider_zero_success(
        &self,
        provider: &InferenceProvider,
        now: Instant,
    ) -> bool {
        let provider_key = provider.to_string();
        let entries = self.entries.lock().expect("health registry");
        let mut qualifying = 0u32;
        let mut all_zero = true;
        for ((prov, _), entry) in entries.iter() {
            if prov != &provider_key {
                continue;
            }
            if now.duration_since(entry.window_started) > WINDOW {
                continue;
            }
            if entry.window.attempts < MIN_WINDOW_ATTEMPTS {
                continue;
            }
            qualifying = qualifying.saturating_add(1);
            if entry.success_rate() > 0.0 {
                all_zero = false;
            }
        }
        qualifying > 0 && all_zero
    }

    /// Pod-lifetime dead credential: ≥10 attempts with zero successes in the
    /// rolling window.
    #[must_use]
    pub fn credential_zero_success_dead(
        &self,
        provider: &InferenceProvider,
        credential: &ProviderCredentialId,
        now: Instant,
    ) -> bool {
        const DEAD_CREDENTIAL_MIN_ATTEMPTS: u32 = 10;
        let entries = self.entries.lock().expect("health registry");
        let Some(entry) =
            entries.get(&(provider.to_string(), credential.to_string()))
        else {
            return false;
        };
        if now.duration_since(entry.window_started) > WINDOW {
            return false;
        }
        entry.window.attempts >= DEAD_CREDENTIAL_MIN_ATTEMPTS
            && entry.success_rate() == 0.0
    }

    /// Seeds a stale rolling window for integration tests (`feature =
    /// "testing"` only).
    #[cfg(feature = "testing")]
    pub fn testing_seed_stale_window(
        &self,
        provider: &InferenceProvider,
        credential: &ProviderCredentialId,
        attempts: u32,
        successes: u32,
    ) {
        let key = (provider.to_string(), credential.to_string());
        let mut entries = self.entries.lock().expect("health registry");
        entries.insert(
            key,
            CredentialHealthEntry {
                window: WindowCounts {
                    attempts,
                    successes,
                },
                window_started: Instant::now()
                    .checked_sub(WINDOW + Duration::from_secs(1))
                    .expect("test window seed"),
                circuit_open_until: None,
                last_failover_class: None,
            },
        );
    }
}

impl CredentialHealthEntry {
    fn success_rate(&self) -> f64 {
        if self.window.attempts == 0 {
            return 1.0;
        }
        f64::from(self.window.successes) / f64::from(self.window.attempts)
    }
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use super::*;
    use crate::types::provider::InferenceProvider;

    #[test]
    fn routing_health_marks_circuit_open_as_planner_excluded() {
        let registry = CredentialHealthRegistry::new();
        let provider = InferenceProvider::GoogleGemini;
        let credential = ProviderCredentialId::new("gemini-free-2");
        for _ in 0..5 {
            registry.record_attempt(
                &provider,
                &credential,
                CallOutcome::RateLimited,
                429,
            );
        }
        let now = Instant::now();
        let snapshot =
            registry.routing_health_snapshot(&provider, &credential, now);
        assert!(snapshot.circuit_open);
        assert!(snapshot.planner_excluded);
        assert!(snapshot.success_rate < 0.11);
    }
}
