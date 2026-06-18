use super::{
    intent_selection, payload,
    types::{BudgetAwareRouter, BudgetCandidate},
};
use crate::{
    config::router::SourceModelSelection,
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
    let intent = intent_selection::routing_intent_for_request(source_model);
    let limits = &router.app_state.config().provider_limits;
    let budget = PayloadBudgetConfig::default();

    if router.source_model_selection == SourceModelSelection::Intent {
        let mut candidates = router.candidates.as_ref().clone();
        candidates.retain(|candidate| {
            intent_selection::passes_source_selection(
                router,
                source_model,
                candidate,
                requirements,
                intent,
            )
        });
        candidates = intent_selection::order_intent_bands(
            router,
            candidates,
            requirements,
            intent,
        );
        let candidates = payload::filter_payload_capable(
            candidates,
            requirements,
            limits,
            budget,
            |_| true,
        );
        return finish_candidates(
            router,
            candidates,
            requirements,
            source_model,
        );
    }

    let mut candidates = router.candidates.as_ref().clone();
    router.rank_candidates(&mut candidates, requirements, None);

    let candidates = payload::filter_payload_capable(
        candidates,
        requirements,
        limits,
        budget,
        |candidate| {
            intent_selection::passes_source_selection(
                router,
                source_model,
                candidate,
                requirements,
                intent,
            )
        },
    );

    finish_candidates(router, candidates, requirements, source_model)
}

fn finish_candidates(
    router: &BudgetAwareRouter,
    candidates: Vec<BudgetCandidate>,
    requirements: &RequestRequirements,
    source_model: Option<&ModelId>,
) -> Result<Vec<BudgetCandidate>, InternalError> {
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
