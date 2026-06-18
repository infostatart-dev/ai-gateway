use super::{
    intent_selection, selection_mode,
    types::{BudgetAwareRouter, BudgetCandidate, CandidateSelectionMode},
};
use crate::{
    error::internal::InternalError,
    router::capability::{
        RequestRequirements, enrich_requirements_from_source_model,
    },
    types::model_id::ModelId,
};

impl BudgetAwareRouter {
    pub(super) fn ordered_candidates(
        &self,
        requirements: &RequestRequirements,
        source_model: Option<&ModelId>,
    ) -> Result<Vec<BudgetCandidate>, InternalError> {
        let mut requirements = requirements.clone();
        enrich_requirements_from_source_model(&mut requirements, source_model);

        match self.selection_mode {
            CandidateSelectionMode::CapabilityThenBudget => self
                .capability_then_budget_candidates(&requirements, source_model),
            CandidateSelectionMode::BudgetThenCapability => {
                selection_mode::budget_then_capability_candidates(
                    self,
                    &requirements,
                    source_model,
                )
            }
        }
    }

    fn capability_then_budget_candidates(
        &self,
        requirements: &RequestRequirements,
        source_model: Option<&ModelId>,
    ) -> Result<Vec<BudgetCandidate>, InternalError> {
        let intent = intent_selection::routing_intent_for_request(source_model);
        let limits = &self.app_state.config().provider_limits;
        let ladders =
            crate::config::model_ladder::ModelLadderRegistry::default();
        let budget =
            crate::router::token_estimate::PayloadBudgetConfig::default();
        let mut candidates = super::payload::filter_payload_capable(
            self.candidates.as_ref().clone(),
            requirements,
            limits,
            budget,
            |candidate| {
                intent_selection::passes_source_selection(
                    self,
                    source_model,
                    candidate,
                    requirements,
                    intent,
                )
            },
        );
        super::ladder_filter::retain_ladder_eligible(
            &mut candidates,
            limits,
            &ladders,
        );

        if candidates.is_empty() {
            tracing::warn!(
                ?requirements,
                ?source_model,
                "no budget-aware candidate matched request"
            );
            return Err(InternalError::ProviderNotFound);
        }

        if self.source_model_selection
            == crate::config::router::SourceModelSelection::Intent
        {
            candidates = intent_selection::order_intent_bands(
                self,
                candidates,
                requirements,
                intent,
            );
        } else {
            self.rank_candidates(&mut candidates, requirements, None);
        }
        Ok(self.credential_round_robin.balance(candidates))
    }

    pub(super) fn matches_source_model(
        &self,
        source_model: &ModelId,
        candidate: &BudgetCandidate,
        requirements: &RequestRequirements,
    ) -> bool {
        use crate::{
            config::{
                chatgpt_web::is_chatgpt_web, deepseek_web::is_deepseek_web,
            },
            types::model_id::ModelIdWithoutVersion,
        };

        if is_chatgpt_web(&candidate.capability.provider)
            || is_deepseek_web(&candidate.capability.provider)
        {
            return true;
        }

        self.model_mapper
            .map_model_with_requirements(
                source_model,
                &candidate.capability.provider,
                requirements,
            )
            .is_ok_and(|target_model| {
                ModelIdWithoutVersion::from(target_model)
                    == ModelIdWithoutVersion::from(
                        candidate.capability.model.clone(),
                    )
            })
    }
}

#[cfg(all(test, feature = "testing"))]
mod tests {
    use crate::{
        app_state::AppState,
        error::internal::InternalError,
        router::{
            budget_aware::router_with_candidates,
            capability::RequestRequirements,
        },
    };

    #[tokio::test]
    async fn ordered_candidates_empty_returns_provider_not_found() {
        let app_state = AppState::test_default().await;
        let router = router_with_candidates(&app_state, vec![]);
        let requirements = RequestRequirements::default();

        let result = router.ordered_candidates(&requirements, None);

        assert!(matches!(result, Err(InternalError::ProviderNotFound)));
    }
}
