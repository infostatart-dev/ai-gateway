use std::time::Duration;

use super::types::BudgetAwareRouter;
use crate::{
    config::credentials::ProviderCredentialId,
    metrics::router::CooldownEvent,
    router::{
        provider_attempt::{lock_credential_states, smoothed_latency},
        retry_after::cooldown_for_response,
    },
    types::{provider::InferenceProvider, response::Response},
};

impl BudgetAwareRouter {
    pub(super) fn record_success(
        &self,
        credential_id: &ProviderCredentialId,
        provider: &InferenceProvider,
        elapsed: Duration,
    ) {
        let mut states = lock_credential_states(&self.states);
        let state = states.entry(credential_id.clone()).or_default();
        let had_cooldown = state.cooldown_until.is_some();
        state.latency = Some(smoothed_latency(state.latency, elapsed));
        state.cooldown_until = None;
        state.failures = 0;
        if had_cooldown {
            self.app_state.runtime_metrics().record_cooldown_exit(
                &CooldownEvent {
                    router_id: &self.router_id,
                    endpoint_type: self.endpoint_type.as_ref(),
                    strategy: self.strategy,
                    provider,
                    credential: credential_id.as_str(),
                },
            );
        }
    }

    /// Terminal failure on the last candidate (no further failover).
    pub(super) async fn record_failure(
        &self,
        credential_id: &ProviderCredentialId,
        provider: &InferenceProvider,
        response: Response,
        elapsed: Duration,
    ) -> Response {
        let config = self
            .app_state
            .config()
            .provider_limits
            .cooldown_for(provider);
        let (response, cooldown) =
            cooldown_for_response(response, &config).await;
        let _ = self.update_failure_state(credential_id, elapsed, cooldown);
        response
    }

    /// Apply cooldown state after a classified failure. Returns `true` when the
    /// credential newly entered cooldown.
    pub(super) fn update_failure_state(
        &self,
        credential_id: &ProviderCredentialId,
        elapsed: Duration,
        cooldown: Duration,
    ) -> bool {
        let mut credential_states = lock_credential_states(&self.states);
        let state = credential_states.entry(credential_id.clone()).or_default();
        state.latency = Some(smoothed_latency(state.latency, elapsed));
        state.failures = state.failures.saturating_add(1);
        let prev_cooldown = state.cooldown_until;
        state.cooldown_until = Some(std::time::Instant::now() + cooldown);
        prev_cooldown.is_none()
    }
}
