use super::{
    selection_mode,
    types::{BudgetAwareRouter, BudgetCandidate, CandidateSelectionMode},
};
use crate::{
    error::internal::InternalError,
    router::capability::{
        RequestRequirements, enrich_requirements_from_source_model, supports,
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
                .capability_then_budget_candidates(
                    &requirements,
                    source_model,
                ),
            CandidateSelectionMode::BudgetThenCapability => {
                selection_mode::budget_then_capability_candidates(
                    self,
                    requirements,
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
        let mut candidates = self
            .candidates
            .iter()
            .filter(|candidate| {
                supports(requirements, &candidate.capability)
                    && source_model.is_none_or(|source_model| {
                        self.matches_source_model(
                            source_model,
                            candidate,
                            requirements,
                        )
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

    pub(super) fn matches_source_model(
        &self,
        source_model: &ModelId,
        candidate: &BudgetCandidate,
        requirements: &RequestRequirements,
    ) -> bool {
        use crate::types::model_id::ModelIdWithoutVersion;

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
mod autodefault_scenario_tests {
    use std::{str::FromStr, sync::Arc, time::Duration};

    use super::*;
    use crate::{
        app_state::AppState,
        config::router::RouterConfig,
        endpoints::EndpointType,
        router::budget_aware::default_provider_budget_rank,
        types::{provider::InferenceProvider, router::RouterId},
    };

    fn autodefault_providers() -> nonempty_collections::NESet<InferenceProvider>
    {
        nonempty_collections::nes![
            InferenceProvider::Named("opencode".into()),
            InferenceProvider::OpenRouter,
            InferenceProvider::Named("groq".into()),
            InferenceProvider::GoogleGemini,
            InferenceProvider::Anthropic,
        ]
    }

    async fn autodefault_budget_router() -> BudgetAwareRouter {
        let app_state = AppState::test_default().await;
        let mut provider_priorities = indexmap::IndexMap::new();
        provider_priorities
            .insert(InferenceProvider::Named("opencode".into()), 0);
        provider_priorities.insert(InferenceProvider::OpenRouter, 1);
        provider_priorities.insert(InferenceProvider::Named("groq".into()), 2);
        provider_priorities.insert(InferenceProvider::GoogleGemini, 10);
        provider_priorities.insert(InferenceProvider::Anthropic, 20);

        BudgetAwareRouter::new_budget_then_capability(
            app_state,
            RouterId::Named("autodefault".into()),
            Arc::new(RouterConfig::default()),
            &autodefault_providers(),
            &provider_priorities,
            Duration::from_secs(3),
            EndpointType::Chat,
            "budget-aware-capability-after",
        )
        .await
        .expect("autodefault-like router")
    }

    fn gpt_5_mini() -> ModelId {
        ModelId::from_str("openai/gpt-5-mini").expect("source model")
    }

    fn candidate_key(candidate: &BudgetCandidate) -> (InferenceProvider, String) {
        (
            candidate.capability.provider.clone(),
            candidate.capability.model.to_string(),
        )
    }

    #[tokio::test]
    async fn budget_then_capability_json_schema_prefers_cheapest_capable_providers(
    ) {
        let router = autodefault_budget_router().await;
        let requirements = RequestRequirements {
            json_schema_required: true,
            ..RequestRequirements::default()
        };

        let candidates = router
            .ordered_candidates(&requirements, Some(&gpt_5_mini()))
            .expect("capable candidates");

        assert!(
            candidates
                .iter()
                .all(|candidate| candidate.capability.supports_json_schema),
            "capability filter must drop models without json_schema support"
        );

        let ordered: Vec<_> = candidates.iter().map(candidate_key).collect();

        assert_eq!(
            ordered[0].0,
            InferenceProvider::Named("opencode".into()),
            "cheapest provider first: {ordered:?}"
        );
        assert!(
            ordered[0].1.contains("nemotron-3-ultra-free"),
            "json_schema + gpt-5-mini reasoning profile must pick nemotron on opencode, got {}",
            ordered[0].1
        );

        let groq = ordered
            .iter()
            .find(|(provider, _)| {
                *provider == InferenceProvider::Named("groq".into())
            })
            .expect("groq json_schema candidate");
        assert!(
            groq.1.contains("llama-4-scout"),
            "groq must map to structured-output model, got {}",
            groq.1
        );

        assert!(
            !ordered.iter().any(|(_, model)| model.contains("qwen3-32b")),
            "groq/qwen3-32b must not pass json_schema capability filter"
        );

        assert!(
            ordered.windows(2).all(|window| {
                default_provider_budget_rank(&window[0].0)
                    <= default_provider_budget_rank(&window[1].0)
            }),
            "providers must stay sorted by budget rank: {ordered:?}"
        );
    }

    #[tokio::test]
    async fn budget_then_capability_plain_chat_keeps_budget_order_with_mapping(
    ) {
        let router = autodefault_budget_router().await;
        let candidates = router
            .ordered_candidates(&RequestRequirements::default(), Some(&gpt_5_mini()))
            .expect("plain chat candidates");

        let ordered: Vec<_> = candidates.iter().map(candidate_key).collect();

        assert_eq!(
            ordered[0].0,
            InferenceProvider::Named("opencode".into())
        );
        assert!(
            ordered[0].1.contains("nemotron-3-ultra-free"),
            "reasoning profile must pick nemotron (reasoning+json_schema) on opencode, got {}",
            ordered[0].1
        );
        assert!(
            ordered.windows(2).all(|window| {
                default_provider_budget_rank(&window[0].0)
                    <= default_provider_budget_rank(&window[1].0)
            }),
            "budget-first ordering for plain chat: {ordered:?}"
        );
    }
}
