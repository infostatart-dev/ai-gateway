use std::time::Instant;

use super::types::{BudgetAwareRouter, BudgetCandidate};
use crate::router::provider_attempt::{
    ModelCooldownKey, lock_credential_states, lock_model_states,
};

impl BudgetAwareRouter {
    pub(super) async fn wait_for_candidate(
        &self,
        candidate: &BudgetCandidate,
        has_next_candidate: bool,
    ) -> bool {
        let model = candidate.capability.model.to_string();
        let model_key = ModelCooldownKey {
            credential_id: candidate.credential_id.clone(),
            model: model.clone(),
        };
        let now = Instant::now();
        let model_remaining = {
            let model_states = lock_model_states(&self.model_states);
            model_states
                .get(&model_key)
                .and_then(|state| state.cooldown_until)
                .and_then(|until| until.checked_duration_since(now))
        };
        let slot_remaining = {
            let states = lock_credential_states(&self.states);
            states
                .get(&candidate.credential_id)
                .and_then(|state| state.cooldown_until)
                .and_then(|until| until.checked_duration_since(now))
        };
        let pacing_wait = self.pacing_wait(candidate).await;
        let remaining = [model_remaining, slot_remaining, pacing_wait]
            .into_iter()
            .flatten()
            .max();

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

    async fn pacing_wait(
        &self,
        candidate: &BudgetCandidate,
    ) -> Option<std::time::Duration> {
        let gate = self.app_state.upstream_pacing().gate_for(
            &candidate.capability.provider,
            Some(&candidate.credential_id),
            Some(candidate.credential_tier.as_str()),
            Some(&candidate.capability.model.to_string()),
        )?;
        let wait = gate.peek_next_wait(0).await;
        if wait > std::time::Duration::ZERO {
            Some(wait)
        } else {
            None
        }
    }
}
