use std::time::Duration;

use super::types::BudgetAwareRouter;
use crate::{
    router::provider_attempt::{
        cooldown_for_response, lock_states, smoothed_latency,
    },
    types::{provider::InferenceProvider, response::Response},
};

impl BudgetAwareRouter {
    pub(super) fn record_success(
        &self,
        provider: &InferenceProvider,
        elapsed: Duration,
    ) {
        let mut states = lock_states(&self.states);
        let state = states.entry(provider.clone()).or_default();
        state.latency = Some(smoothed_latency(state.latency, elapsed));
        state.cooldown_until = None;
        state.failures = 0;
    }

    pub(super) fn record_failure(
        &self,
        provider: &InferenceProvider,
        response: &Response,
        elapsed: Duration,
    ) {
        let mut states = lock_states(&self.states);
        let state = states.entry(provider.clone()).or_default();
        state.latency = Some(smoothed_latency(state.latency, elapsed));
        state.failures = state.failures.saturating_add(1);
        state.cooldown_until =
            Some(std::time::Instant::now() + cooldown_for_response(response));
    }
}
