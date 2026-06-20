use std::time::Instant;

use crate::{
    config::{
        credentials::ProviderCredentialId,
        provider_limits::ProviderLimitCatalog,
    },
    router::{
        budget_aware::CredentialHealthRegistry,
        pacing::PacingRegistry,
        quota_admission::{AdmissionVerdict, BlockedReason},
    },
    types::provider::InferenceProvider,
};

pub struct PacingAdmissionScope<'a> {
    pub pacing: &'a PacingRegistry,
    pub health: &'a CredentialHealthRegistry,
    pub limits: &'a ProviderLimitCatalog,
    pub provider: &'a InferenceProvider,
    pub credential_id: &'a ProviderCredentialId,
    pub tier: &'a str,
    pub model: Option<&'a str>,
    pub estimated_tokens: u32,
    pub now: Instant,
}

/// Global admission peek (pacing + circuit) for observability snapshots.
pub async fn evaluate_pacing_admission(
    scope: PacingAdmissionScope<'_>,
) -> AdmissionVerdict {
    let PacingAdmissionScope {
        pacing,
        health,
        limits,
        provider,
        credential_id,
        tier,
        model,
        estimated_tokens,
        now,
    } = scope;

    if health.is_circuit_open(provider, credential_id, now) {
        return AdmissionVerdict::from_blocking(
            std::time::Duration::MAX,
            BlockedReason::Circuit,
        );
    }

    let Some(gate) =
        pacing.gate_for(provider, Some(credential_id), Some(tier), model)
    else {
        return AdmissionVerdict::from_blocking(
            std::time::Duration::ZERO,
            BlockedReason::None,
        );
    };

    let reconcile_wait = gate.upstream_reconcile_wait(now).await;
    if reconcile_wait > std::time::Duration::ZERO {
        return AdmissionVerdict::from_blocking(
            reconcile_wait,
            BlockedReason::UpstreamReconcile,
        );
    }

    let pacing_wait = gate.peek_next_wait(estimated_tokens).await;
    if pacing_wait > std::time::Duration::ZERO {
        return AdmissionVerdict::from_blocking(
            pacing_wait,
            pacing_block_reason(gate.limits()),
        );
    }

    if !gate.daily_headroom_available(estimated_tokens).await {
        let wait = gate.daily_reset_wait().await;
        let reason = if model.is_some_and(|slug| {
            limits
                .resolve_model_limits(provider, tier, slug)
                .is_some_and(|r| {
                    matches!(
                        r.limits.tpd,
                        crate::config::provider_limits::QuotaValue::Limited(_)
                    )
                })
        }) {
            BlockedReason::Tpd
        } else {
            BlockedReason::Rpd
        };
        return AdmissionVerdict::from_blocking(wait, reason);
    }

    AdmissionVerdict::from_blocking(
        std::time::Duration::ZERO,
        BlockedReason::None,
    )
}

fn pacing_block_reason(
    limits: &crate::router::pacing::limits::PacingLimits,
) -> BlockedReason {
    if limits.min_interval > std::time::Duration::ZERO {
        BlockedReason::MinInterval
    } else if limits.tpm.is_some() {
        BlockedReason::Tpm
    } else {
        BlockedReason::Rpm
    }
}
