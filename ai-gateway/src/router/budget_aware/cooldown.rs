use std::time::Instant;

use super::types::{BudgetAwareRouter, BudgetCandidate};
use crate::router::provider_attempt::lock_credential_states;

impl BudgetAwareRouter {
    pub(super) async fn wait_for_candidate(
        &self,
        candidate: &BudgetCandidate,
        has_next_candidate: bool,
    ) -> bool {
        let remaining = {
            let states = lock_credential_states(&self.states);
            states
                .get(&candidate.credential_id)
                .and_then(|state| state.cooldown_until)
                .and_then(|until| until.checked_duration_since(Instant::now()))
        };

        let Some(remaining) = remaining else {
            return true;
        };
        if remaining <= self.max_cooldown_wait {
            tracing::debug!(
                credential = %candidate.credential_id,
                provider = %candidate.capability.provider,
                model = %candidate.capability.model,
                wait_ms = remaining.as_millis(),
                "waiting for cheap budget-aware candidate cooldown"
            );
            tokio::time::sleep(remaining).await;
            return true;
        }

        if has_next_candidate {
            tracing::debug!(
                credential = %candidate.credential_id,
                provider = %candidate.capability.provider,
                model = %candidate.capability.model,
                cooldown_ms = remaining.as_millis(),
                "skipping candidate with cooldown above budget wait"
            );
            return false;
        }

        tracing::debug!(
            credential = %candidate.credential_id,
            provider = %candidate.capability.provider,
            model = %candidate.capability.model,
            wait_ms = remaining.as_millis(),
            "waiting full cooldown for sole provider candidate"
        );
        tokio::time::sleep(remaining).await;
        true
    }
}
