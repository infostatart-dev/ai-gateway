use super::types::{BudgetAwareRouter, BudgetCandidate};
use crate::{
    error::internal::InternalError,
    router::capability::{RequestRequirements, supports},
    types::model_id::ModelId,
};

pub(super) fn budget_then_capability_candidates(
    router: &BudgetAwareRouter,
    requirements: &RequestRequirements,
    source_model: Option<&ModelId>,
) -> Result<Vec<BudgetCandidate>, InternalError> {
    let mut candidates = router.candidates.as_ref().clone();
    router.rank_candidates(&mut candidates, requirements);

    let candidates = candidates
        .into_iter()
        .filter(|candidate| {
            supports(requirements, &candidate.capability)
                && source_model.is_none_or(|source_model| {
                    router.matches_source_model(
                        source_model,
                        candidate,
                        requirements,
                    )
                })
        })
        .collect::<Vec<_>>();

    if candidates.is_empty() {
        tracing::warn!(
            ?requirements,
            ?source_model,
            "no budget-then-capability candidate matched request"
        );
        return Err(InternalError::ProviderNotFound);
    }

    Ok(candidates)
}
