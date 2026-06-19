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
    credential_tier: &str,
) -> (Response, FailoverClass, ExhaustionScope) {
    let limits = &router.app_state.config().provider_limits;
    let config = limits.cooldown_for(provider);
    let quota_profile = limits.quota_profile(provider);
    let status = response.status();
    let (response, mut cooldown, class, scope, slot_cooldown) =
        classify_and_cooldown(response, &config, quota_profile).await;
    let pacing_wait = router
        .app_state
        .upstream_pacing()
        .gate_for(
            provider,
            Some(credential_id),
            Some(credential_tier),
            Some(model),
        )
        .map(|gate| async move { gate.peek_next_wait(0).await });
    if let Some(wait_future) = pacing_wait {
        cooldown = cooldown.max(wait_future.await);
    }
    router.app_state.credential_health().record_failover_class(
        provider,
        credential_id,
        class,
        scope,
    );
    if status == http::StatusCode::PAYMENT_REQUIRED
        && scope == ExhaustionScope::Project
    {
        router
            .app_state
            .budget_probe()
            .record_payment_required(provider, credential_id);
    }
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

    use super::record_classified_failure;
    use crate::{
        app_state::AppState,
        router::{
            budget_aware::{
                openrouter_model_candidate, router_with_candidates,
            },
            retry_after::ExhaustionScope,
        },
        tests::routing_harness::responses::openrouter_never_purchased_402,
        types::provider::InferenceProvider,
    };

    #[tokio::test]
    async fn record_failover_metric_accepts_transient_class() {
        use super::record_failover_metric;
        use crate::router::{
            budget_aware::{gemini_model_candidate, router_with_candidates},
            retry_after::FailoverClass,
        };

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

    #[tokio::test]
    async fn unpaid_402_model_scope_does_not_poison_free_budget_probe() {
        let app_state = AppState::test_default().await;
        let candidate = openrouter_model_candidate(
            &app_state,
            "openrouter-default",
            "openai/gpt-4o-mini",
        )
        .await;
        let router =
            router_with_candidates(&app_state, vec![candidate.clone()]);
        let (_, _, scope) = record_classified_failure(
            &router,
            &candidate.credential_id,
            &candidate.capability.provider,
            &candidate.capability.model.to_string(),
            openrouter_never_purchased_402(),
            std::time::Duration::from_millis(5),
            candidate.credential_tier.as_str(),
        )
        .await;
        assert_eq!(scope, ExhaustionScope::Model);
        let credentials = app_state.config().credentials.clone();
        assert!(
            !router
                .app_state
                .budget_probe()
                .should_skip_candidate(
                    &credentials,
                    &InferenceProvider::OpenRouter,
                    &candidate.credential_id,
                    "openai/gpt-oss-120b:free",
                )
                .await
        );
    }
}
