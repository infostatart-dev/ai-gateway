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
        Ok(self.credential_round_robin.balance(candidates))
    }

    pub(super) fn matches_source_model(
        &self,
        source_model: &ModelId,
        candidate: &BudgetCandidate,
        requirements: &RequestRequirements,
    ) -> bool {
        use crate::{
            config::chatgpt_web::is_chatgpt_web,
            types::model_id::ModelIdWithoutVersion,
        };

        if is_chatgpt_web(&candidate.capability.provider) {
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
mod autodefault_scenario_tests {
    use std::{str::FromStr, sync::Arc, time::Duration};

    use super::*;
    use crate::{
        app_state::AppState,
        config::router::RouterConfig,
        endpoints::EndpointType,
        types::{provider::InferenceProvider, router::RouterId},
    };

    fn autodefault_providers() -> nonempty_collections::NESet<InferenceProvider>
    {
        nonempty_collections::nes![
            InferenceProvider::Named("opencode".into()),
            InferenceProvider::OpenRouter,
            InferenceProvider::Named("mistral".into()),
            InferenceProvider::Named("groq".into()),
            InferenceProvider::Named("cerebras".into()),
            InferenceProvider::Named("cloudflare".into()),
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
        provider_priorities
            .insert(InferenceProvider::Named("mistral".into()), 2);
        provider_priorities.insert(InferenceProvider::Named("groq".into()), 3);
        provider_priorities
            .insert(InferenceProvider::Named("cerebras".into()), 4);
        provider_priorities
            .insert(InferenceProvider::Named("cloudflare".into()), 5);
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

    fn candidate_key(
        candidate: &BudgetCandidate,
    ) -> (InferenceProvider, String) {
        (
            candidate.capability.provider.clone(),
            candidate.capability.model.to_string(),
        )
    }

    #[tokio::test]
    async fn budget_then_capability_json_schema_prefers_cheapest_capable_providers()
     {
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
            "json_schema + gpt-5-mini reasoning profile must pick nemotron on \
             opencode, got {}",
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

        let cerebras = ordered
            .iter()
            .find(|(provider, _)| {
                *provider == InferenceProvider::Named("cerebras".into())
            })
            .expect("cerebras json_schema candidate");
        assert!(
            cerebras.1.contains("gpt-oss-120b"),
            "cerebras must map to gpt-oss-120b for gpt-5-mini json_schema, \
             got {}",
            cerebras.1
        );

        let mistral = ordered
            .iter()
            .find(|(provider, model)| {
                *provider == InferenceProvider::Named("mistral".into())
                    && model.contains("magistral-medium-latest")
            })
            .expect("mistral json_schema candidate");
        assert!(
            mistral.1.contains("magistral-medium-latest"),
            "mistral must map to magistral-medium-latest for gpt-5-mini \
             json_schema+reasoning, got {}",
            mistral.1
        );

        let cloudflare = ordered
            .iter()
            .find(|(provider, _)| {
                *provider == InferenceProvider::Named("cloudflare".into())
            })
            .expect("cloudflare json_schema candidate");
        assert!(
            cloudflare.1.contains("deepseek-r1-distill-qwen-32b"),
            "cloudflare must map to reasoning+json_schema model for \
             gpt-5-mini, got {}",
            cloudflare.1
        );

        assert!(
            !ordered.iter().any(|(_, model)| model.contains("qwen3-32b")),
            "groq/qwen3-32b must not pass json_schema capability filter"
        );

        assert!(
            ordered.windows(2).all(|window| {
                let left = candidates
                    .iter()
                    .find(|c| candidate_key(c) == window[0])
                    .expect("left candidate");
                let right = candidates
                    .iter()
                    .find(|c| candidate_key(c) == window[1])
                    .expect("right candidate");
                router.budget_rank(left) <= router.budget_rank(right)
            }),
            "providers must stay sorted by budget rank: {ordered:?}"
        );
    }

    #[tokio::test]
    async fn budget_then_capability_plain_chat_keeps_budget_order_with_mapping()
    {
        let router = autodefault_budget_router().await;
        let candidates = router
            .ordered_candidates(
                &RequestRequirements::default(),
                Some(&gpt_5_mini()),
            )
            .expect("plain chat candidates");

        let ordered: Vec<_> = candidates.iter().map(candidate_key).collect();

        assert_eq!(ordered[0].0, InferenceProvider::Named("opencode".into()));
        assert!(
            ordered[0].1.contains("nemotron-3-ultra-free"),
            "reasoning profile must pick nemotron (reasoning+json_schema) on \
             opencode, got {}",
            ordered[0].1
        );
        assert!(
            ordered.windows(2).all(|window| {
                let left = candidates
                    .iter()
                    .find(|c| candidate_key(c) == window[0])
                    .expect("left candidate");
                let right = candidates
                    .iter()
                    .find(|c| candidate_key(c) == window[1])
                    .expect("right candidate");
                router.budget_rank(left) <= router.budget_rank(right)
            }),
            "budget-first ordering for plain chat: {ordered:?}"
        );
    }
}
