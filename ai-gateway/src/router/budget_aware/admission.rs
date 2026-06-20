use std::time::Instant;

use super::types::{BudgetAwareRouter, BudgetCandidate};
use crate::router::{
    budget_aware::CredentialHealthRegistry,
    pacing::PacingRegistry,
    provider_attempt::{
        ModelCooldownKey, lock_credential_states, lock_model_states,
    },
    quota_admission::{AdmissionVerdict, BlockedReason},
};

impl BudgetAwareRouter {
    pub(crate) async fn evaluate_admission(
        &self,
        pacing: &PacingRegistry,
        health: &CredentialHealthRegistry,
        candidate: &BudgetCandidate,
        estimated_tokens: u32,
        now: Instant,
    ) -> AdmissionVerdict {
        if health.is_circuit_open(
            &candidate.capability.provider,
            &candidate.credential_id,
            now,
        ) {
            return AdmissionVerdict::from_blocking(
                std::time::Duration::MAX,
                BlockedReason::Circuit,
            );
        }

        let model = candidate.capability.model.to_string();
        let model_key = ModelCooldownKey {
            credential_id: candidate.credential_id.clone(),
            model: model.clone(),
        };
        let model_remaining = {
            let states = lock_model_states(&self.model_states);
            states
                .get(&model_key)
                .and_then(|state| state.cooldown_until)
                .and_then(|until| until.checked_duration_since(now))
        };
        if let Some(wait) = model_remaining.filter(|w| !w.is_zero()) {
            return AdmissionVerdict::from_blocking(
                wait,
                BlockedReason::ModelCooldown,
            );
        }

        let slot_remaining = {
            let states = lock_credential_states(&self.states);
            states
                .get(&candidate.credential_id)
                .and_then(|state| state.cooldown_until)
                .and_then(|until| until.checked_duration_since(now))
        };
        if let Some(wait) = slot_remaining.filter(|w| !w.is_zero()) {
            return AdmissionVerdict::from_blocking(
                wait,
                BlockedReason::SlotCooldown,
            );
        }

        let limits = &self.app_state.config().provider_limits;
        let gate = pacing.gate_for(
            &candidate.capability.provider,
            Some(&candidate.credential_id),
            Some(candidate.credential_tier.as_str()),
            Some(model.as_str()),
        );
        let Some(gate) = gate else {
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
            let reason = pacing_block_reason(gate.limits());
            return AdmissionVerdict::from_blocking(pacing_wait, reason);
        }

        if !gate.daily_headroom_available(estimated_tokens).await {
            let wait = gate.daily_reset_wait().await;
            let reason = daily_block_reason(limits, candidate);
            return AdmissionVerdict::from_blocking(wait, reason);
        }

        AdmissionVerdict::from_blocking(
            std::time::Duration::ZERO,
            BlockedReason::None,
        )
    }
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

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::*;
    use crate::tests::routing::{PacingGate, PacingLimits};

    #[tokio::test]
    async fn catalog_rpd_exhaustion_blocks_admission() {
        let gate = PacingGate::new(PacingLimits {
            concurrent: 4,
            rpm: u32::MAX,
            tpm: None,
            rpd: Some(1),
            tpd: None,
            daily_reset_utc_hour: 0,
            min_interval: Duration::ZERO,
            max_queue_wait: Duration::from_secs(1),
        });
        gate.acquire(0).await.unwrap();
        assert!(!gate.daily_headroom_available(0).await);
        let wait = gate.daily_reset_wait().await;
        assert!(wait > Duration::ZERO);
    }

    #[tokio::test]
    async fn reconcile_wait_blocks_peek() {
        let gate = PacingGate::new(PacingLimits {
            concurrent: 4,
            rpm: u32::MAX,
            tpm: None,
            rpd: None,
            tpd: None,
            daily_reset_utc_hour: 0,
            min_interval: Duration::ZERO,
            max_queue_wait: Duration::from_secs(1),
        });
        gate.apply_upstream_reconcile(Instant::now() + Duration::from_secs(30))
            .await;
        assert!(gate.peek_next_wait(0).await > Duration::from_secs(25));
    }
}

fn daily_block_reason(
    limits: &crate::config::provider_limits::ProviderLimitCatalog,
    candidate: &BudgetCandidate,
) -> BlockedReason {
    let resolved = crate::config::catalog_limit_resolve::catalog_limit_resolve(
        limits,
        &candidate.capability.provider,
        candidate.credential_tier.as_str(),
        &candidate.capability.model.to_string(),
    );
    if resolved.as_ref().is_some_and(|r| {
        matches!(
            r.limits.tpd,
            crate::config::provider_limits::QuotaValue::Limited(_)
        )
    }) {
        BlockedReason::Tpd
    } else {
        BlockedReason::Rpd
    }
}
