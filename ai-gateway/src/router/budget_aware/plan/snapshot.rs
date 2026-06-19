use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

use chrono::Utc;

use super::super::types::BudgetCandidate;
use crate::{
    config::catalog_limit_resolve::normalize_model_slug,
    router::{budget_aware::types::BudgetAwareRouter, pacing::PacingRegistry},
};

#[derive(Debug, Clone)]
pub struct QuotaSnapshotEntry {
    pub next_wait: Duration,
    pub headroom_score: f64,
}

#[derive(Debug, Clone)]
pub struct QuotaSnapshot {
    captured_at: chrono::DateTime<Utc>,
    entries: HashMap<(String, String), QuotaSnapshotEntry>,
}

impl QuotaSnapshot {
    pub async fn capture(
        pacing: &PacingRegistry,
        health: &crate::router::budget_aware::CredentialHealthRegistry,
        _router: &BudgetAwareRouter,
        candidates: &[BudgetCandidate],
        estimated_tokens: u32,
        max_cooldown_wait: Duration,
        now: Instant,
    ) -> Self {
        let mut entries = HashMap::new();
        for candidate in candidates {
            let model = candidate.capability.model.to_string();
            let slug = normalize_model_slug(&model);
            let key = (candidate.credential_id.to_string(), slug.clone());
            if entries.contains_key(&key) {
                continue;
            }
            if health.is_circuit_open(
                &candidate.capability.provider,
                &candidate.credential_id,
                now,
            ) {
                entries.insert(
                    key,
                    QuotaSnapshotEntry {
                        next_wait: Duration::MAX,
                        headroom_score: 0.0,
                    },
                );
                continue;
            }
            let gate = pacing.gate_for(
                &candidate.capability.provider,
                Some(&candidate.credential_id),
                Some(candidate.credential_tier.as_str()),
                Some(slug.as_str()),
            );
            let (next_wait, daily_ok) = if let Some(gate) = gate {
                let wait = gate.peek_next_wait(estimated_tokens).await;
                let daily =
                    gate.daily_headroom_available(estimated_tokens).await;
                (wait, daily)
            } else {
                (Duration::ZERO, true)
            };
            let headroom_score = if !daily_ok || next_wait > max_cooldown_wait {
                0.0
            } else {
                1.0 / (1.0 + next_wait.as_secs_f64())
            };
            entries.insert(
                key,
                QuotaSnapshotEntry {
                    next_wait,
                    headroom_score,
                },
            );
        }
        Self {
            captured_at: Utc::now(),
            entries,
        }
    }

    #[must_use]
    pub fn headroom_score(&self, credential_id: &str, model: &str) -> f64 {
        let slug = normalize_model_slug(model);
        self.entries
            .get(&(credential_id.to_string(), slug))
            .map_or(1.0, |entry| entry.headroom_score)
    }

    #[must_use]
    pub fn next_wait(&self, credential_id: &str, model: &str) -> Duration {
        let slug = normalize_model_slug(model);
        self.entries
            .get(&(credential_id.to_string(), slug))
            .map_or(Duration::ZERO, |entry| entry.next_wait)
    }

    #[must_use]
    pub fn captured_at(&self) -> chrono::DateTime<Utc> {
        self.captured_at
    }
}
