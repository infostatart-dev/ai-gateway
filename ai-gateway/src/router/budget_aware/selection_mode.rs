use super::{
    payload,
    types::{BudgetAwareRouter, BudgetCandidate},
};
use crate::{
    error::internal::InternalError,
    router::{
        capability::RequestRequirements, token_estimate::PayloadBudgetConfig,
    },
    types::model_id::ModelId,
};

pub(super) fn budget_then_capability_candidates(
    router: &BudgetAwareRouter,
    requirements: &RequestRequirements,
    source_model: Option<&ModelId>,
) -> Result<Vec<BudgetCandidate>, InternalError> {
    let mut candidates = router.candidates.as_ref().clone();
    router.rank_candidates(&mut candidates, requirements);

    let limits = &router.app_state.config().provider_limits;
    let budget = PayloadBudgetConfig::default();
    let candidates = payload::filter_payload_capable(
        candidates,
        requirements,
        limits,
        budget,
        |candidate| {
            source_model.is_none_or(|model| {
                router.matches_source_model(model, candidate, requirements)
            })
        },
    );

    if candidates.is_empty() {
        tracing::warn!(
            ?requirements,
            ?source_model,
            "no budget-then-capability candidate matched request"
        );
        return Err(InternalError::ProviderNotFound);
    }

    Ok(router.credential_round_robin.balance(candidates))
}
