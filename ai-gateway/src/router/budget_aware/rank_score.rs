use std::time::Instant;

use super::{rank, types::BudgetAwareRouter};
use crate::router::provider_attempt::{
    ModelCooldownKey, ProviderState, lock_model_states,
};

impl BudgetAwareRouter {
    pub(super) fn effective_budget_rank(
        &self,
        candidate: &super::types::BudgetCandidate,
        state: Option<&ProviderState>,
        now: Instant,
    ) -> u16 {
        let base = self.budget_rank(candidate);
        let model_key = ModelCooldownKey {
            credential_id: candidate.credential_id.clone(),
            model: candidate.capability.model.to_string(),
        };
        let model_states = lock_model_states(&self.model_states);
        let model_remaining = model_states
            .get(&model_key)
            .and_then(|state| state.cooldown_until)
            .and_then(|until| until.checked_duration_since(now));
        drop(model_states);
        let slot_remaining = state
            .and_then(|state| state.cooldown_until)
            .and_then(|until| until.checked_duration_since(now));
        let remaining_cooldown = match (slot_remaining, model_remaining) {
            (Some(a), Some(b)) => Some(a.max(b)),
            (Some(a), None) => Some(a),
            (None, Some(b)) => Some(b),
            (None, None) => None,
        };

        rank::effective_budget_rank(
            base,
            remaining_cooldown,
            self.max_cooldown_wait,
        )
    }

    pub(crate) fn budget_rank(
        &self,
        candidate: &super::types::BudgetCandidate,
    ) -> u16 {
        let cost_base = candidate.credential_cost_class.rank_base();
        let provider_rank = self
            .provider_priorities
            .get(&candidate.capability.provider)
            .copied()
            .unwrap_or_else(|| {
                rank::default_budget_rank(&candidate.capability)
            });
        cost_base
            .saturating_add(candidate.credential_budget_rank.saturating_mul(10))
            .saturating_add(provider_rank)
    }
}

#[cfg(all(test, feature = "testing"))]
mod tests {
    use std::time::Instant;

    use crate::{
        app_state::AppState,
        router::{
            budget_aware::test_support::gemini_model_candidate,
            provider_attempt::lock_model_states,
        },
    };

    #[tokio::test]
    async fn model_cooldown_deprioritizes_over_fresh_slot() {
        let app_state = AppState::test_default().await;
        let candidate = gemini_model_candidate(
            &app_state,
            "gemini-free-1",
            "gemini-3-flash-preview",
        )
        .await;
        let router = crate::router::budget_aware::empty_router(&app_state);
        {
            let mut model_states = lock_model_states(&router.model_states);
            model_states.insert(
                crate::router::provider_attempt::ModelCooldownKey {
                    credential_id: candidate.credential_id.clone(),
                    model: candidate.capability.model.to_string(),
                },
                crate::router::provider_attempt::ProviderState {
                    cooldown_until: Some(
                        Instant::now() + std::time::Duration::from_secs(120),
                    ),
                    ..Default::default()
                },
            );
        }
        let now = Instant::now();
        let fresh = gemini_model_candidate(
            &app_state,
            "gemini-free-2",
            "gemini-3-flash-preview",
        )
        .await;
        let hot = router.effective_budget_rank(&candidate, None, now);
        let cool = router.effective_budget_rank(&fresh, None, now);
        assert!(cool < hot, "model cooldown should increase effective rank");
    }
}
