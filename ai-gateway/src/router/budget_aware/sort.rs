use std::time::Instant;

use super::types::BudgetAwareRouter;
use crate::router::{
    capability::RequestRequirements, provider_attempt::lock_states,
};

impl BudgetAwareRouter {
    pub(super) fn rank_candidates(
        &self,
        candidates: &mut [super::types::BudgetCandidate],
        requirements: &RequestRequirements,
    ) {
        let now = Instant::now();
        let states = lock_states(&self.states);

        candidates.sort_by(|left, right| {
            let left_state = states.get(&left.capability.provider);
            let right_state = states.get(&right.capability.provider);

            self.effective_budget_rank(left, left_state, now)
                .cmp(&self.effective_budget_rank(right, right_state, now))
                .then_with(|| {
                    let left_reasoning = left.capability.reasoning
                        == requirements.reasoning_preferred;
                    let right_reasoning = right.capability.reasoning
                        == requirements.reasoning_preferred;
                    right_reasoning.cmp(&left_reasoning)
                })
                .then_with(|| {
                    let left_failures = left_state.map_or(0, |s| s.failures);
                    let right_failures = right_state.map_or(0, |s| s.failures);
                    left_failures.cmp(&right_failures)
                })
                .then_with(|| {
                    let left_latency = left_state
                        .and_then(|s| s.latency)
                        .unwrap_or(self.default_latency);
                    let right_latency = right_state
                        .and_then(|s| s.latency)
                        .unwrap_or(self.default_latency);
                    left_latency.cmp(&right_latency)
                })
                .then_with(|| {
                    left.capability
                        .model
                        .to_string()
                        .cmp(&right.capability.model.to_string())
                })
        });
    }
}
