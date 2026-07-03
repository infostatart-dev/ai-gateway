use std::time::{Duration, Instant};

use super::types::{BudgetAwareRouter, BudgetCandidate};
use crate::router::quota_admission::BlockedReason;

pub(super) enum CandidateWaitOutcome {
    Ready,
    Skipped {
        wait: Duration,
        blocked_reason: BlockedReason,
    },
    Waited {
        wait: Duration,
        blocked_reason: BlockedReason,
    },
}

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
    ) -> CandidateWaitOutcome {
        let verdict = self.admit_candidate(candidate, 0, Instant::now()).await;
        if verdict.feasible {
            return CandidateWaitOutcome::Ready;
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
            return CandidateWaitOutcome::Skipped {
                wait: verdict.next_wait,
                blocked_reason: verdict.blocked_reason,
            };
        }

        if verdict.next_wait <= self.max_cooldown_wait {
            tracing::debug!(
                credential = %candidate.credential_id,
                provider = %candidate.capability.provider,
                model = %candidate.capability.model,
                wait_ms = verdict.next_wait.as_millis(),
                "waiting for terminal candidate admission"
            );
            tracing::event!(
                tracing::Level::INFO,
                blocked_reason = ?verdict.blocked_reason,
                wait_ms = u64::try_from(verdict.next_wait.as_millis())
                    .unwrap_or(u64::MAX),
                provider = %candidate.capability.provider,
                credential = %candidate.credential_id,
                model = %candidate.capability.model,
                "gateway.pacing.wait"
            );
            tokio::time::sleep(verdict.next_wait).await;
            return CandidateWaitOutcome::Waited {
                wait: verdict.next_wait,
                blocked_reason: verdict.blocked_reason,
            };
        }

        tracing::debug!(
            credential = %candidate.credential_id,
            provider = %candidate.capability.provider,
            model = %candidate.capability.model,
            wait_ms = verdict.next_wait.as_millis(),
            "waiting full admission block for sole candidate"
        );
        tracing::event!(
            tracing::Level::INFO,
            blocked_reason = ?verdict.blocked_reason,
            wait_ms = u64::try_from(verdict.next_wait.as_millis())
                .unwrap_or(u64::MAX),
            provider = %candidate.capability.provider,
            credential = %candidate.credential_id,
            model = %candidate.capability.model,
            "gateway.pacing.wait"
        );
        tokio::time::sleep(verdict.next_wait).await;
        CandidateWaitOutcome::Waited {
            wait: verdict.next_wait,
            blocked_reason: verdict.blocked_reason,
        }
    }
}
