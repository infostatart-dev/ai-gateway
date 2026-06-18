//! Acceptance matrix A–D for autodefault intent routing.

#[cfg(all(test, feature = "testing"))]
mod acceptance {
    use std::{str::FromStr, sync::Arc, time::Duration};

    use super::super::{
        credential_balance::CredentialRoundRobin,
        types::{BudgetAwareRouter, BudgetCandidate, CandidateSelectionMode},
    };
    use crate::{
        app_state::AppState,
        config::{
            credentials::ProviderCredentialId,
            router::{RouterConfig, SourceModelSelection},
        },
        dispatcher::Dispatcher,
        endpoints::EndpointType,
        middleware::mapper::model::ModelMapper,
        router::{
            capability::{ModelCapability, RequestRequirements},
            intent::{IntentTier, extract_routing_intent_from_name},
        },
        types::{
            model_id::ModelId, provider::InferenceProvider, router::RouterId,
        },
    };

    async fn intent_router(
        app_state: &AppState,
        candidates: Vec<BudgetCandidate>,
    ) -> BudgetAwareRouter {
        BudgetAwareRouter {
            app_state: app_state.clone(),
            router_id: RouterId::Named("autodefault".into()),
            endpoint_type: EndpointType::Chat,
            strategy: "budget-aware-capability-after",
            candidates: Arc::new(candidates),
            model_mapper: ModelMapper::new_for_router(
                app_state.clone(),
                Arc::new(RouterConfig {
                    source_model_selection: Some(SourceModelSelection::Intent),
                    ..Default::default()
                }),
            ),
            states: Arc::new(std::sync::Mutex::new(
                std::collections::HashMap::new(),
            )),
            provider_priorities: Arc::new(indexmap::IndexMap::new()),
            default_latency: Duration::from_millis(10),
            max_cooldown_wait: Duration::from_secs(0),
            selection_mode: CandidateSelectionMode::BudgetThenCapability,
            credential_round_robin: CredentialRoundRobin::new_shared(),
            source_model_selection: SourceModelSelection::Intent,
        }
    }

    async fn candidate(
        app_state: &AppState,
        capability: ModelCapability,
        credential: &str,
    ) -> BudgetCandidate {
        let router_id = RouterId::Named("intent-acceptance".into());
        let router_config = Arc::new(RouterConfig::default());
        let service = Dispatcher::new_with_model_id_without_rate_limit_events(
            app_state.clone(),
            &router_id,
            &router_config,
            capability.provider.clone(),
            capability.model.clone(),
        )
        .await
        .expect("dispatcher");
        BudgetCandidate {
            credential_id: ProviderCredentialId::new(credential),
            credential_budget_rank: 0,
            credential_cost_class: crate::config::cost_class::CostClass::Free,
            credential_tier: "free".into(),
            capability,
            service,
        }
    }

    fn scout() -> ModelCapability {
        ModelCapability {
            provider: InferenceProvider::Named("groq".into()),
            model: ModelId::from_str(
                "groq/meta-llama/llama-4-scout-17b-16e-instruct",
            )
            .expect("model"),
            context_window: Some(131_072),
            supports_tools: true,
            supports_json_schema: true,
            supports_vision: false,
            reasoning: false,
            json_schema_rank: 1,
            intent_tier: IntentTier::FastThinking,
        }
    }

    fn plain_fast() -> ModelCapability {
        ModelCapability {
            provider: InferenceProvider::Named("cloudflare".into()),
            model: ModelId::from_str(
                "cloudflare/@cf/meta/llama-3.2-3b-instruct",
            )
            .expect("model"),
            context_window: Some(128_000),
            supports_tools: true,
            supports_json_schema: false,
            supports_vision: false,
            reasoning: false,
            json_schema_rank: 0,
            intent_tier: IntentTier::Fast,
        }
    }

    fn deep() -> ModelCapability {
        ModelCapability {
            provider: InferenceProvider::Anthropic,
            model: ModelId::from_str("anthropic/claude-sonnet-4-0")
                .expect("model"),
            context_window: Some(200_000),
            supports_tools: true,
            supports_json_schema: true,
            supports_vision: true,
            reasoning: true,
            json_schema_rank: 2,
            intent_tier: IntentTier::Deep,
        }
    }

    fn source(model: &str) -> ModelId {
        ModelId::from_str(model).expect("source model")
    }

    #[tokio::test]
    async fn acceptance_a_mini_json_strict_prefers_fast_thinking() {
        let app_state = AppState::test_default().await;
        let router = intent_router(
            &app_state,
            vec![
                candidate(&app_state, deep(), "anthropic-test").await,
                candidate(&app_state, scout(), "groq-test").await,
            ],
        )
        .await;
        let requirements = RequestRequirements {
            json_schema_required: true,
            ..RequestRequirements::default()
        };
        let ordered = router
            .ordered_candidates(
                &requirements,
                Some(&source("openai/gpt-5-mini")),
            )
            .expect("candidates");
        assert_eq!(ordered[0].capability.intent_tier, IntentTier::FastThinking);
    }

    #[tokio::test]
    async fn acceptance_b_mini_plain_includes_non_json() {
        let app_state = AppState::test_default().await;
        let router = intent_router(
            &app_state,
            vec![
                candidate(&app_state, plain_fast(), "cloudflare-test").await,
                candidate(&app_state, scout(), "groq-test").await,
            ],
        )
        .await;
        let ordered = router
            .ordered_candidates(
                &RequestRequirements::default(),
                Some(&source("openai/gpt-5-mini")),
            )
            .expect("candidates");
        assert!(ordered.iter().any(|c| !c.capability.supports_json_schema));
    }

    #[tokio::test]
    async fn acceptance_c_nano_json_strict_matches_mini() {
        assert_eq!(
            extract_routing_intent_from_name("openai/gpt-5-mini"),
            extract_routing_intent_from_name("openai/gpt-5-nano")
        );
        let app_state = AppState::test_default().await;
        let router = intent_router(
            &app_state,
            vec![
                candidate(&app_state, deep(), "anthropic-test").await,
                candidate(&app_state, scout(), "groq-test").await,
            ],
        )
        .await;
        let requirements = RequestRequirements {
            json_schema_required: true,
            ..RequestRequirements::default()
        };
        let ordered = router
            .ordered_candidates(
                &requirements,
                Some(&source("openai/gpt-5-nano")),
            )
            .expect("candidates");
        assert_eq!(ordered[0].capability.intent_tier, IntentTier::FastThinking);
    }

    #[tokio::test]
    async fn acceptance_d_nano_plain_matches_mini() {
        let app_state = AppState::test_default().await;
        let router = intent_router(
            &app_state,
            vec![
                candidate(&app_state, plain_fast(), "cloudflare-test").await,
                candidate(&app_state, scout(), "groq-test").await,
            ],
        )
        .await;
        let ordered = router
            .ordered_candidates(
                &RequestRequirements::default(),
                Some(&source("openai/gpt-5-nano")),
            )
            .expect("candidates");
        assert!(ordered.iter().any(|c| !c.capability.supports_json_schema));
    }

    #[tokio::test]
    async fn deep_gpt5_excludes_scout_even_when_free() {
        let app_state = AppState::test_default().await;
        let router = intent_router(
            &app_state,
            vec![
                candidate(&app_state, scout(), "groq-test").await,
                candidate(&app_state, deep(), "anthropic-test").await,
            ],
        )
        .await;
        let ordered = router
            .ordered_candidates(
                &RequestRequirements::default(),
                Some(&source("openai/gpt-5")),
            )
            .expect("candidates");
        assert!(
            ordered
                .iter()
                .all(|c| c.capability.intent_tier == IntentTier::Deep)
        );
    }

    #[tokio::test]
    async fn json_strict_excludes_non_json_upstream() {
        let app_state = AppState::test_default().await;
        let router = intent_router(
            &app_state,
            vec![
                candidate(&app_state, plain_fast(), "cloudflare-test").await,
                candidate(&app_state, scout(), "groq-test").await,
            ],
        )
        .await;
        let requirements = RequestRequirements {
            json_schema_required: true,
            ..RequestRequirements::default()
        };
        let ordered = router
            .ordered_candidates(
                &requirements,
                Some(&source("openai/gpt-5-mini")),
            )
            .expect("candidates");
        assert!(ordered.iter().all(|c| c.capability.supports_json_schema));
    }

    #[tokio::test]
    async fn fast_thinking_orders_deep_in_escalation_band() {
        let app_state = AppState::test_default().await;
        let router = intent_router(
            &app_state,
            vec![
                candidate(&app_state, scout(), "groq-test").await,
                candidate(&app_state, deep(), "anthropic-test").await,
            ],
        )
        .await;
        let requirements = RequestRequirements {
            json_schema_required: true,
            ..RequestRequirements::default()
        };
        let ordered = router
            .ordered_candidates(
                &requirements,
                Some(&source("openai/gpt-5-nano")),
            )
            .expect("candidates");
        assert_eq!(ordered.len(), 2);
        assert_eq!(ordered[0].capability.intent_tier, IntentTier::FastThinking);
        assert_eq!(ordered[1].capability.intent_tier, IntentTier::Deep);
    }

    #[tokio::test]
    async fn strict_mode_preserves_mapping_gate() {
        let app_state = AppState::test_default().await;
        let candidates = vec![
            candidate(&app_state, scout(), "groq-test").await,
            candidate(&app_state, deep(), "anthropic-test").await,
        ];
        let router = BudgetAwareRouter {
            app_state: app_state.clone(),
            router_id: RouterId::Named("strict-regression".into()),
            endpoint_type: EndpointType::Chat,
            strategy: "budget-aware-capability-after",
            candidates: Arc::new(candidates),
            model_mapper: ModelMapper::new_for_router(
                app_state.clone(),
                Arc::new(RouterConfig {
                    source_model_selection: Some(SourceModelSelection::Strict),
                    ..Default::default()
                }),
            ),
            states: Arc::new(std::sync::Mutex::new(
                std::collections::HashMap::new(),
            )),
            provider_priorities: Arc::new(indexmap::IndexMap::new()),
            default_latency: Duration::from_millis(10),
            max_cooldown_wait: Duration::from_secs(0),
            selection_mode: CandidateSelectionMode::BudgetThenCapability,
            credential_round_robin: CredentialRoundRobin::new_shared(),
            source_model_selection: SourceModelSelection::Strict,
        };
        let requirements = RequestRequirements::default();
        let source = source("openai/gpt-5-mini");
        let ordered = router
            .ordered_candidates(&requirements, Some(&source))
            .expect("candidates");
        for candidate in &ordered {
            assert!(router.matches_source_model(
                &source,
                candidate,
                &requirements
            ));
        }
        assert!(
            ordered
                .iter()
                .any(|c| c.credential_id.as_str() == "groq-test"),
            "strict mode still routes via model-mapping bindings"
        );
    }
}
