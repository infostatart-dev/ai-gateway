#[cfg(all(test, feature = "testing"))]
mod tests {
    use std::{
        collections::HashMap,
        sync::{Arc, Mutex},
        time::Duration,
    };

    use axum_core::body::Body;
    use bytes::Bytes;
    use http::{Request, StatusCode};
    use indexmap::IndexMap;

    use super::super::{
        call::{clear_test_call_responses, push_test_call_response},
        credential_balance::CredentialRoundRobin,
        failover_loop::run_failover_candidates,
        types::{BudgetAwareRouter, BudgetCandidate, CandidateSelectionMode},
    };
    use crate::{
        app_state::AppState,
        config::{credentials::ProviderCredentialId, router::RouterConfig},
        dispatcher::Dispatcher,
        endpoints::EndpointType,
        middleware::mapper::model::ModelMapper,
        router::{
            capability::{ModelCapability, RequestRequirements},
            routed_identity::REAL_MODE_MODEL_AND_PROVIDER,
        },
        types::{
            model_id::ModelId,
            provider::{InferenceProvider, ProviderKey},
            router::RouterId,
            secret::Secret,
        },
    };

    fn rate_limited() -> crate::types::response::Response {
        http::Response::builder()
            .status(StatusCode::TOO_MANY_REQUESTS)
            .body(Body::from(r#"{"error":"rate limit"}"#))
            .unwrap()
    }

    fn daily_quota_exhausted() -> crate::types::response::Response {
        http::Response::builder()
            .status(StatusCode::TOO_MANY_REQUESTS)
            .body(Body::from(
                r#"{"error":{"message":"You exceeded your daily limit."}}"#,
            ))
            .unwrap()
    }

    fn overload_503() -> crate::types::response::Response {
        http::Response::builder()
            .status(StatusCode::SERVICE_UNAVAILABLE)
            .body(Body::from("model is overloaded"))
            .unwrap()
    }

    fn ok_response() -> crate::types::response::Response {
        http::Response::builder()
            .status(StatusCode::OK)
            .header(http::header::CONTENT_TYPE, "application/json")
            .body(Body::from(r#"{"choices":[{"message":{"content":"ok"}}]}"#))
            .unwrap()
    }

    fn test_router(app_state: &AppState) -> BudgetAwareRouter {
        BudgetAwareRouter {
            app_state: app_state.clone(),
            router_id: RouterId::Named("credential-failover-test".into()),
            endpoint_type: EndpointType::Chat,
            strategy: "budget-aware-capability-after",
            candidates: Arc::new(vec![]),
            model_mapper: ModelMapper::new_for_router(
                app_state.clone(),
                Arc::new(RouterConfig::default()),
            ),
            states: Arc::new(Mutex::new(HashMap::new())),
            provider_priorities: Arc::new(IndexMap::new()),
            default_latency: Duration::from_millis(10),
            max_cooldown_wait: Duration::from_secs(0),
            selection_mode: CandidateSelectionMode::BudgetThenCapability,
            credential_round_robin: CredentialRoundRobin::new_shared(),
        }
    }

    async fn gemini_candidate(
        app_state: &AppState,
        credential_id: &str,
        budget_rank: u16,
        key: &str,
    ) -> BudgetCandidate {
        let provider = InferenceProvider::GoogleGemini;
        let router_id = RouterId::Named("credential-failover-test".into());
        let router_config = Arc::new(RouterConfig::default());
        let cred = ProviderCredentialId::new(credential_id);
        let model_id = ModelId::from_str_and_provider(
            provider.clone(),
            "gemini-2.5-flash",
        )
        .unwrap();
        let service = Dispatcher::new_with_model_id_and_provider_key_without_rate_limit_events(
            app_state.clone(),
            &router_id,
            &router_config,
            provider.clone(),
            model_id.clone(),
            Some(&ProviderKey::Secret(Secret::from(key.to_string()))),
            Some(&cred),
        )
        .await
        .expect("dispatcher");

        BudgetCandidate {
            credential_id: cred,
            credential_budget_rank: budget_rank,
            credential_cost_class: if budget_rank == 0 {
                crate::config::cost_class::CostClass::Free
            } else {
                crate::config::cost_class::CostClass::Paid
            },
            credential_tier: if budget_rank == 0 {
                "free".into()
            } else {
                "tier-3".into()
            },
            capability: ModelCapability {
                provider,
                model: model_id,
                context_window: Some(1_000_000),
                supports_tools: true,
                supports_json_schema: true,
                supports_vision: true,
                reasoning: false,
                json_schema_rank: 2,
            },
            service,
        }
    }

    async fn anthropic_candidate(app_state: &AppState) -> BudgetCandidate {
        let provider = InferenceProvider::Anthropic;
        let router_id = RouterId::Named("credential-failover-test".into());
        let router_config = Arc::new(RouterConfig::default());
        let cred = ProviderCredentialId::new("anthropic-default");
        let model_id =
            ModelId::from_str_and_provider(provider.clone(), "claude-sonnet")
                .unwrap();
        let service = Dispatcher::new_with_model_id_and_provider_key_without_rate_limit_events(
            app_state.clone(),
            &router_id,
            &router_config,
            provider.clone(),
            model_id.clone(),
            Some(&ProviderKey::Secret(Secret::from(
                "anthropic-key".to_string(),
            ))),
            Some(&cred),
        )
        .await
        .expect("dispatcher");

        BudgetCandidate {
            credential_id: cred,
            credential_budget_rank: 0,
            credential_cost_class: crate::config::cost_class::CostClass::Paid,
            credential_tier: "tier-3".into(),
            capability: ModelCapability {
                provider,
                model: model_id,
                context_window: Some(200_000),
                supports_tools: true,
                supports_json_schema: true,
                supports_vision: true,
                reasoning: false,
                json_schema_rank: 2,
            },
            service,
        }
    }

    fn request_parts() -> http::request::Parts {
        Request::builder()
            .method(http::Method::POST)
            .uri("/v1/chat/completions")
            .body(())
            .unwrap()
            .into_parts()
            .0
    }

    fn routed_identity(response: &crate::types::response::Response) -> String {
        response
            .headers()
            .get(REAL_MODE_MODEL_AND_PROVIDER)
            .expect("routed identity header")
            .to_str()
            .unwrap()
            .to_string()
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn gemini_daily_quota_skips_remaining_free_siblings_to_paid() {
        clear_test_call_responses();
        push_test_call_response(Ok(daily_quota_exhausted()));
        push_test_call_response(Ok(ok_response()));

        let app_state = AppState::test_default().await;
        let router = test_router(&app_state);
        let candidates = router.credential_round_robin.balance(vec![
            gemini_candidate(&app_state, "gemini-free", 0, "free-key").await,
            gemini_candidate(&app_state, "gemini-free-2", 0, "free-2-key")
                .await,
            gemini_candidate(&app_state, "gemini-free-3", 0, "free-3-key")
                .await,
            gemini_candidate(&app_state, "gemini-default", 10, "paid-key")
                .await,
        ]);

        let response = run_failover_candidates(
            router,
            request_parts(),
            Bytes::from(r#"{"model":"gpt-4o-mini","messages":[]}"#),
            candidates,
            RequestRequirements::default(),
        )
        .await
        .expect("failover reaches paid after quota skip");

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            routed_identity(&response),
            "gemini-default/gemini-2.5-flash"
        );
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn gemini_overload_skips_remaining_free_siblings_to_paid() {
        clear_test_call_responses();
        push_test_call_response(Ok(overload_503()));
        push_test_call_response(Ok(ok_response()));

        let app_state = AppState::test_default().await;
        let router = test_router(&app_state);
        let candidates = router.credential_round_robin.balance(vec![
            gemini_candidate(&app_state, "gemini-free", 0, "free-key").await,
            gemini_candidate(&app_state, "gemini-free-2", 0, "free-2-key")
                .await,
            gemini_candidate(&app_state, "gemini-default", 10, "paid-key")
                .await,
        ]);

        let response = run_failover_candidates(
            router,
            request_parts(),
            Bytes::from(r#"{"model":"gpt-4o-mini","messages":[]}"#),
            candidates,
            RequestRequirements::default(),
        )
        .await
        .expect("failover reaches paid after overload skip");

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            routed_identity(&response),
            "gemini-default/gemini-2.5-flash"
        );
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn gemini_free_429_failover_to_sibling_free_slot() {
        clear_test_call_responses();
        push_test_call_response(Ok(rate_limited()));
        push_test_call_response(Ok(ok_response()));

        let app_state = AppState::test_default().await;
        let router = test_router(&app_state);
        let candidates = router.credential_round_robin.balance(vec![
            gemini_candidate(&app_state, "gemini-free", 0, "free-key").await,
            gemini_candidate(&app_state, "gemini-free-2", 0, "free-2-key")
                .await,
            gemini_candidate(&app_state, "gemini-default", 10, "paid-key")
                .await,
        ]);

        let response = run_failover_candidates(
            router,
            request_parts(),
            Bytes::from(r#"{"model":"gpt-4o-mini","messages":[]}"#),
            candidates,
            RequestRequirements::default(),
        )
        .await
        .expect("failover succeeds on sibling free credential");

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            routed_identity(&response),
            "gemini-free-2/gemini-2.5-flash"
        );
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn gemini_free_429_failover_to_paid_credential() {
        clear_test_call_responses();
        push_test_call_response(Ok(rate_limited()));
        push_test_call_response(Ok(ok_response()));

        let app_state = AppState::test_default().await;
        let router = test_router(&app_state);
        let candidates = router.credential_round_robin.balance(vec![
            gemini_candidate(&app_state, "gemini-free", 0, "free-key").await,
            gemini_candidate(&app_state, "gemini-default", 10, "paid-key")
                .await,
        ]);

        let response = run_failover_candidates(
            router,
            request_parts(),
            Bytes::from(r#"{"model":"gpt-4o-mini","messages":[]}"#),
            candidates,
            RequestRequirements::default(),
        )
        .await
        .expect("failover succeeds on second credential");

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            routed_identity(&response),
            "gemini-default/gemini-2.5-flash"
        );
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn alternates_gemini_accounts_on_success() {
        clear_test_call_responses();
        for _ in 0..4 {
            push_test_call_response(Ok(ok_response()));
        }

        let app_state = AppState::test_default().await;
        let router = test_router(&app_state);
        let ranked = vec![
            gemini_candidate(&app_state, "gemini-free", 0, "free-key").await,
            gemini_candidate(&app_state, "gemini-default", 10, "paid-key")
                .await,
        ];
        let parts = request_parts();
        let body = Bytes::from(r#"{"model":"gpt-4o-mini","messages":[]}"#);

        let mut identities = Vec::new();
        for _ in 0..4 {
            let candidates =
                router.credential_round_robin.balance(ranked.clone());
            let response = run_failover_candidates(
                test_router(&app_state),
                parts.clone(),
                body.clone(),
                candidates,
                RequestRequirements::default(),
            )
            .await
            .expect("success");
            identities.push(routed_identity(&response));
        }

        assert_eq!(
            identities
                .iter()
                .filter(|id| id.starts_with("gemini-free/"))
                .count(),
            2
        );
        assert_eq!(
            identities
                .iter()
                .filter(|id| id.starts_with("gemini-default/"))
                .count(),
            2
        );
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn groups_gemini_accounts_before_cross_provider_failover() {
        clear_test_call_responses();
        push_test_call_response(Ok(rate_limited()));
        push_test_call_response(Ok(ok_response()));

        let app_state = AppState::test_default().await;
        let router = test_router(&app_state);
        let scattered = vec![
            gemini_candidate(&app_state, "gemini-free", 0, "free-key").await,
            anthropic_candidate(&app_state).await,
            gemini_candidate(&app_state, "gemini-default", 10, "paid-key")
                .await,
        ];
        let candidates = router.credential_round_robin.balance(scattered);

        let ids: Vec<_> = candidates
            .iter()
            .map(|c| c.credential_id.to_string())
            .collect();
        assert_eq!(
            ids,
            vec!["gemini-free", "gemini-default", "anthropic-default"]
        );

        let response = run_failover_candidates(
            router,
            request_parts(),
            Bytes::from(r#"{"model":"gpt-4o-mini","messages":[]}"#),
            candidates,
            RequestRequirements::default(),
        )
        .await
        .expect("paid gemini succeeds before anthropic");

        assert_eq!(
            routed_identity(&response),
            "gemini-default/gemini-2.5-flash"
        );
    }
}
