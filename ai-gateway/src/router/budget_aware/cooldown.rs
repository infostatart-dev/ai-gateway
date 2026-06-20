use std::time::Instant;

use super::types::{BudgetAwareRouter, BudgetCandidate};

impl BudgetAwareRouter {
    pub(super) async fn admit_candidate(
        &self,
        candidate: &BudgetCandidate,
        estimated_tokens: u32,
        now: Instant,
    ) -> crate::router::quota_admission::AdmissionVerdict {
        self.evaluate_admission(
            self.app_state.upstream_pacing(),
            self.app_state.credential_health(),
            candidate,
            estimated_tokens,
            now,
        )
        .await
    }

    pub(super) async fn wait_for_candidate(
        &self,
        candidate: &BudgetCandidate,
        has_next_candidate: bool,
    ) -> bool {
        let verdict = self.admit_candidate(candidate, 0, Instant::now()).await;
        if verdict.feasible {
            return true;
        }
        if has_next_candidate {
            tracing::debug!(
                credential = %candidate.credential_id,
                provider = %candidate.capability.provider,
                model = %candidate.capability.model,
                wait_ms = verdict.next_wait.as_millis(),
                blocked_reason = ?verdict.blocked_reason,
                "skipping infeasible planned hop"
            );
            return false;
        }

        if verdict.next_wait <= self.max_cooldown_wait {
            tracing::debug!(
                credential = %candidate.credential_id,
                provider = %candidate.capability.provider,
                model = %candidate.capability.model,
                wait_ms = verdict.next_wait.as_millis(),
                "waiting for terminal candidate admission"
            );
            tokio::time::sleep(verdict.next_wait).await;
            return true;
        }

        tracing::debug!(
            credential = %candidate.credential_id,
            provider = %candidate.capability.provider,
            model = %candidate.capability.model,
            wait_ms = verdict.next_wait.as_millis(),
            "waiting full admission block for sole candidate"
        );
        tokio::time::sleep(verdict.next_wait).await;
        true
    }
}
