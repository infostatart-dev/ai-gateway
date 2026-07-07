use std::time::Duration;

use super::types::BudgetAwareRouter;
use crate::{
    config::credentials::ProviderCredentialId,
    metrics::{provider::attempt::CallOutcome, router::CooldownEvent},
    router::{
        provider_attempt::{
            ModelCooldownKey, lock_credential_states, lock_model_states,
            smoothed_latency,
        },
        retry_after::{ExhaustionScope, cooldown_for_response},
    },
    types::{provider::InferenceProvider, response::Response},
};

impl BudgetAwareRouter {
    pub(super) fn record_success(
        &self,
        credential_id: &ProviderCredentialId,
        provider: &InferenceProvider,
        model: &str,
        elapsed: Duration,
    ) {
        self.app_state.credential_health().record_model_attempt(
            provider,
            credential_id,
            model,
            CallOutcome::Success,
            http::StatusCode::OK.as_u16(),
            elapsed,
        );

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
        drop(states);

        let mut model_states = lock_model_states(&self.model_states);
        if let Some(state) = model_states.get_mut(&ModelCooldownKey {
            credential_id: credential_id.clone(),
            model: model.to_string(),
        }) {
            state.cooldown_until = None;
            state.failures = 0;
        }
    }

    pub(super) fn record_success_degraded(
        &self,
        credential_id: &ProviderCredentialId,
        provider: &InferenceProvider,
        model: &str,
        elapsed: Duration,
    ) {
        self.app_state.credential_health().record_model_attempt(
            provider,
            credential_id,
            model,
            CallOutcome::SuccessDegraded,
            http::StatusCode::OK.as_u16(),
            elapsed,
        );
    }

    /// Terminal failure on the last candidate (no further failover).
    pub(super) async fn record_failure(
        &self,
        credential_id: &ProviderCredentialId,
        provider: &InferenceProvider,
        model: &str,
        response: Response,
        elapsed: Duration,
    ) -> Response {
        let status = response.status();
        self.app_state.credential_health().record_model_attempt(
            provider,
            credential_id,
            model,
            failure_outcome(status),
            status.as_u16(),
            elapsed,
        );
        let config = self
            .app_state
            .config()
            .provider_limits
            .cooldown_for(provider);
        let profile = self
            .app_state
            .config()
            .provider_limits
            .quota_profile(provider);
        let (response, cooldown, scope) =
            cooldown_for_response(response, &config, profile).await;
        let _ = self.update_failure_state_scoped(
            credential_id,
            model,
            scope,
            elapsed,
            cooldown,
        );
        response
    }

    /// Apply cooldown state after a classified failure. Returns `true` when the
    /// target newly entered cooldown.
    pub(super) fn update_failure_state_scoped(
        &self,
        credential_id: &ProviderCredentialId,
        model: &str,
        scope: ExhaustionScope,
        elapsed: Duration,
        cooldown: Duration,
    ) -> bool {
        match scope {
            ExhaustionScope::Model => self.update_model_failure_state(
                credential_id,
                model,
                elapsed,
                cooldown,
            ),
            ExhaustionScope::Slot | ExhaustionScope::Project => {
                self.update_failure_state(credential_id, elapsed, cooldown)
            }
        }
    }

    fn update_model_failure_state(
        &self,
        credential_id: &ProviderCredentialId,
        model: &str,
        elapsed: Duration,
        cooldown: Duration,
    ) -> bool {
        let mut model_states = lock_model_states(&self.model_states);
        let key = ModelCooldownKey {
            credential_id: credential_id.clone(),
            model: model.to_string(),
        };
        let state = model_states.entry(key).or_default();
        state.latency = Some(smoothed_latency(state.latency, elapsed));
        state.failures = state.failures.saturating_add(1);
        let prev_cooldown = state.cooldown_until;
        state.cooldown_until = Some(std::time::Instant::now() + cooldown);
        prev_cooldown.is_none()
    }

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

fn failure_outcome(status: http::StatusCode) -> CallOutcome {
    if status == http::StatusCode::TOO_MANY_REQUESTS {
        return CallOutcome::RateLimited;
    }
    if status.is_server_error() {
        return CallOutcome::ServerError;
    }
    CallOutcome::ClientError
}
