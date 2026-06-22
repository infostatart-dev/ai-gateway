use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

use chrono::{DateTime, Utc};

use super::super::types::BudgetCandidate;
use crate::{
    config::catalog_limit_resolve::normalize_model_slug,
    router::{
        budget_aware::types::BudgetAwareRouter,
        pacing::PacingRegistry,
        quota_admission::{
            AdmissionVerdict, BlockedReason, evaluate_candidate,
        },
    },
};

#[derive(Debug, Clone)]
pub struct QuotaSnapshotEntry {
    pub next_wait: Duration,
    pub headroom_score: f64,
    pub blocked_reason: BlockedReason,
    pub next_available_at: Option<DateTime<Utc>>,
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
        router: &BudgetAwareRouter,
        candidates: &[BudgetCandidate],
        estimated_tokens: u32,
        _max_cooldown_wait: Duration,
        now: Instant,
    ) -> Self {
        let limits = &router.app_state.config().provider_limits;
        let mut entries = HashMap::new();
        for candidate in candidates {
            let model = candidate.capability.model.to_string();
            let slug = normalize_model_slug(&model);
            let key = (candidate.credential_id.to_string(), slug.clone());
            if entries.contains_key(&key) {
                continue;
            }
            let verdict = evaluate_candidate(
                pacing,
                health,
                limits,
                router,
                candidate,
                estimated_tokens,
                now,
            )
            .await;
            entries.insert(key, entry_from_verdict(&verdict));
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
    pub fn blocked_reason(
        &self,
        credential_id: &str,
        model: &str,
    ) -> BlockedReason {
        let slug = normalize_model_slug(model);
        self.entries
            .get(&(credential_id.to_string(), slug))
            .map_or(BlockedReason::None, |entry| entry.blocked_reason)
    }

    #[must_use]
    pub fn next_available_at(
        &self,
        credential_id: &str,
        model: &str,
    ) -> Option<DateTime<Utc>> {
        let slug = normalize_model_slug(model);
        self.entries
            .get(&(credential_id.to_string(), slug))
            .and_then(|entry| entry.next_available_at)
    }

    #[must_use]
    pub fn captured_at(&self) -> chrono::DateTime<Utc> {
        self.captured_at
    }
}

fn entry_from_verdict(verdict: &AdmissionVerdict) -> QuotaSnapshotEntry {
    QuotaSnapshotEntry {
        next_wait: verdict.next_wait,
        headroom_score: verdict.headroom_score(),
        blocked_reason: verdict.blocked_reason,
        next_available_at: verdict.next_available_at,
    }
}
