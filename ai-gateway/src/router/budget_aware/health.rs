use std::time::Duration;

use super::types::BudgetAwareRouter;
use crate::{
    config::credentials::ProviderCredentialId,
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
                &self.router_id,
                self.endpoint_type.as_ref(),
                self.strategy,
                provider,
            );
        }
    }

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
        let status = response.status();
        let (response, cooldown) =
            cooldown_for_response(response, &config).await;
        let mut credential_states = lock_credential_states(&self.states);
        let state = credential_states.entry(credential_id.clone()).or_default();
        state.latency = Some(smoothed_latency(state.latency, elapsed));
        state.failures = state.failures.saturating_add(1);
        let prev_cooldown = state.cooldown_until;
        state.cooldown_until = Some(std::time::Instant::now() + cooldown);
        if prev_cooldown.is_none() {
            self.app_state.runtime_metrics().record_cooldown_enter(
                &self.router_id,
                self.endpoint_type.as_ref(),
                self.strategy,
                provider,
                crate::metrics::router::status_class(status),
            );
        }
        response
    }
}
