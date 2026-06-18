//! Failover-path failure handling: classify upstream outcome, update credential
//! state, and emit observability. Keeps [`super::health`] focused on state
//! only.

use std::time::Duration;

use super::types::BudgetAwareRouter;
use crate::{
    config::credentials::ProviderCredentialId,
    metrics::router::{CooldownEvent, FailoverEvent},
    router::retry_after::{
        ExhaustionScope, FailoverClass, classify_and_cooldown,
        quota_metric_label,
    },
    types::{provider::InferenceProvider, response::Response},
};

/// Classify `response`, apply cooldown state, and emit cooldown metrics.
pub(super) async fn record_classified_failure(
    router: &BudgetAwareRouter,
    credential_id: &ProviderCredentialId,
    provider: &InferenceProvider,
    model: &str,
    response: Response,
    elapsed: Duration,
) -> (Response, FailoverClass, ExhaustionScope) {
    let config = router
        .app_state
        .config()
        .provider_limits
        .cooldown_for(provider);
    let status = response.status();
    if status == http::StatusCode::PAYMENT_REQUIRED {
        router
            .app_state
            .budget_probe()
            .record_payment_required(provider, credential_id);
    }
    let (response, cooldown, class, scope) =
        classify_and_cooldown(response, &config).await;
    let entered_cooldown = router.update_failure_state_scoped(
        credential_id,
        model,
        scope,
        elapsed,
        cooldown,
    );
    if entered_cooldown {
        router.app_state.runtime_metrics().record_cooldown_enter(
            &CooldownEvent {
                router_id: &router.router_id,
                endpoint_type: router.endpoint_type.as_ref(),
                strategy: router.strategy,
                provider,
                credential: credential_id.as_str(),
            },
            crate::metrics::router::status_class(status),
            quota_metric_label(status, class),
        );
    }
    (response, class, scope)
}

pub(super) fn record_failover_metric(
    router: &BudgetAwareRouter,
    candidate: &super::types::BudgetCandidate,
    next_provider: Option<&InferenceProvider>,
    reason: &str,
    status: http::StatusCode,
    class: FailoverClass,
) {
    router
        .app_state
        .runtime_metrics()
        .record_failover(&FailoverEvent {
            router_id: &router.router_id,
            endpoint_type: router.endpoint_type.as_ref(),
            strategy: router.strategy,
            from_provider: &candidate.capability.provider,
            to_provider: next_provider,
            reason,
            credential: candidate.credential_id.as_str(),
            quota_metric: quota_metric_label(status, class),
        });
}
