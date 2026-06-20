use std::time::Instant;

use super::verdict::AdmissionVerdict;
use crate::router::budget_aware::{
    BudgetAwareRouter, BudgetCandidate, CredentialHealthRegistry,
};

pub async fn evaluate_candidate(
    pacing: &crate::router::pacing::PacingRegistry,
    health: &CredentialHealthRegistry,
    _limits: &crate::config::provider_limits::ProviderLimitCatalog,
    router: &BudgetAwareRouter,
    candidate: &BudgetCandidate,
    estimated_tokens: u32,
    now: Instant,
) -> AdmissionVerdict {
    router
        .evaluate_admission(pacing, health, candidate, estimated_tokens, now)
        .await
}
