use super::types::{BudgetAwareRouter, BudgetCandidate};
use crate::{
    error::internal::InternalError,
    router::capability::{RequestRequirements, supports},
    types::model_id::ModelId,
};

impl BudgetAwareRouter {
    pub(super) fn ordered_candidates(
        &self,
        requirements: &RequestRequirements,
        source_model: Option<&ModelId>,
    ) -> Result<Vec<BudgetCandidate>, InternalError> {
        let mut candidates = self
            .candidates
            .iter()
            .filter(|candidate| {
                supports(requirements, &candidate.capability)
                    && source_model.is_none_or(|source_model| {
                        self.matches_source_model(source_model, candidate)
                    })
            })
            .cloned()
            .collect::<Vec<_>>();

        if candidates.is_empty() {
            tracing::warn!(
                ?requirements,
                ?source_model,
                "no budget-aware candidate matched request"
            );
            return Err(InternalError::ProviderNotFound);
        }

        self.rank_candidates(&mut candidates, requirements);
        Ok(candidates)
    }

    fn matches_source_model(
        &self,
        source_model: &ModelId,
        candidate: &BudgetCandidate,
    ) -> bool {
        use crate::types::model_id::ModelIdWithoutVersion;

        self.model_mapper
            .map_model(source_model, &candidate.capability.provider)
            .is_ok_and(|target_model| {
                ModelIdWithoutVersion::from(target_model)
                    == ModelIdWithoutVersion::from(
                        candidate.capability.model.clone(),
                    )
            })
    }
}
