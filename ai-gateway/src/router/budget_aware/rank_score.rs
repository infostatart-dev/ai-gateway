use std::time::Instant;

use super::{rank, types::BudgetAwareRouter};
use crate::router::provider_attempt::ProviderState;

impl BudgetAwareRouter {
    pub(super) fn effective_budget_rank(
        &self,
        candidate: &super::types::BudgetCandidate,
        state: Option<&ProviderState>,
        now: Instant,
    ) -> u16 {
        let base = self.budget_rank(candidate);
        let remaining_cooldown = state
            .and_then(|state| state.cooldown_until)
            .and_then(|until| until.checked_duration_since(now));

        rank::effective_budget_rank(
            base,
            remaining_cooldown,
            self.max_cooldown_wait,
        )
    }

    pub(super) fn budget_rank(
        &self,
        candidate: &super::types::BudgetCandidate,
    ) -> u16 {
        self.provider_priorities
            .get(&candidate.capability.provider)
            .copied()
            .unwrap_or_else(|| rank::default_budget_rank(&candidate.capability))
    }
}
