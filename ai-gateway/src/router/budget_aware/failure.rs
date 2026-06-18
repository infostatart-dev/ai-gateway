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
    let limits = &router.app_state.config().provider_limits;
    let config = limits.cooldown_for(provider);
    let quota_profile = limits.quota_profile(provider);
    let status = response.status();
    if status == http::StatusCode::PAYMENT_REQUIRED {
        router
            .app_state
            .budget_probe()
            .record_payment_required(provider, credential_id);
    }
    let (response, cooldown, class, scope, slot_cooldown) =
        classify_and_cooldown(response, &config, quota_profile).await;
    let entered_cooldown = router.update_failure_state_scoped(
        credential_id,
        model,
        scope,
        elapsed,
        cooldown,
    );
    if let Some(slot_cooldown) = slot_cooldown {
        router.update_failure_state(credential_id, elapsed, slot_cooldown);
    }
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

#[cfg(all(test, feature = "testing"))]
mod tests {
    use http::StatusCode;

    use super::record_failover_metric;
    use crate::{
        app_state::AppState,
        router::{
            budget_aware::{gemini_model_candidate, router_with_candidates},
            retry_after::FailoverClass,
        },
        types::provider::InferenceProvider,
    };

    #[tokio::test]
    async fn record_failover_metric_accepts_transient_class() {
        let app_state = AppState::test_default().await;
        let candidate = gemini_model_candidate(
            &app_state,
            "gemini-free",
            "gemini-2.0-flash",
        )
        .await;
        let router =
            router_with_candidates(&app_state, vec![candidate.clone()]);

        record_failover_metric(
            &router,
            &candidate,
            Some(&InferenceProvider::GoogleGemini),
            "rpm_exhausted",
            StatusCode::TOO_MANY_REQUESTS,
            FailoverClass::Transient,
        );
    }
}
