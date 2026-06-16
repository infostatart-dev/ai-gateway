use std::{cmp::Ordering, time::Instant};

use super::types::BudgetAwareRouter;
use crate::router::{
    capability::{RequestRequirements, capability_fit_score},
    provider_attempt::lock_credential_states,
};

impl BudgetAwareRouter {
    pub(super) fn rank_candidates(
        &self,
        candidates: &mut [super::types::BudgetCandidate],
        requirements: &RequestRequirements,
    ) {
        let now = Instant::now();
        let states = lock_credential_states(&self.states);

        candidates.sort_by(|left, right| {
            let left_state = states.get(&left.credential_id);
            let right_state = states.get(&right.credential_id);

            self.effective_budget_rank(left, left_state, now)
                .cmp(&self.effective_budget_rank(right, right_state, now))
                .then_with(|| {
                    if requirements.json_schema_required {
                        right
                            .capability
                            .json_schema_rank
                            .cmp(&left.capability.json_schema_rank)
                    } else {
                        Ordering::Equal
                    }
                })
                .then_with(|| {
                    capability_fit_score(requirements, &right.capability).cmp(
                        &capability_fit_score(requirements, &left.capability),
                    )
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
