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

    fn ok_response() -> crate::types::response::Response {
        http::Response::builder()
            .status(StatusCode::OK)
            .header(http::header::CONTENT_TYPE, "application/json")
            .body(Body::from(r#"{"choices":[{"message":{"content":"ok"}}]}"#))
            .unwrap()
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
        )
        .await
        .expect("dispatcher");

        BudgetCandidate {
            credential_id: ProviderCredentialId::new(credential_id),
            credential_budget_rank: budget_rank,
            capability: ModelCapability {
                provider,
                model: model_id,
                context_window: Some(1_000_000),
                supports_tools: true,
                supports_json_schema: true,
                supports_vision: true,
                reasoning: false,
            },
            service,
        }
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn gemini_free_429_failover_to_paid_credential() {
        clear_test_call_responses();
        push_test_call_response(Ok(rate_limited()));
        push_test_call_response(Ok(ok_response()));

        let app_state = AppState::test_default().await;
        let router = BudgetAwareRouter {
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
        };

        let candidates = vec![
            gemini_candidate(&app_state, "gemini-free", 0, "free-key").await,
            gemini_candidate(&app_state, "gemini-default", 10, "paid-key")
                .await,
        ];

        let parts = Request::builder()
            .method(http::Method::POST)
            .uri("/v1/chat/completions")
            .body(())
            .unwrap()
            .into_parts()
            .0;

        let response = run_failover_candidates(
            router,
            parts,
            Bytes::from(r#"{"model":"gpt-4o-mini","messages":[]}"#),
            candidates,
            RequestRequirements::default(),
        )
        .await
        .expect("failover succeeds on second credential");

        assert_eq!(response.status(), StatusCode::OK);
        let header = response
            .headers()
            .get(REAL_MODE_MODEL_AND_PROVIDER)
            .expect("routed identity header");
        assert_eq!(header.to_str().unwrap(), "gemini-default/gemini-2.5-flash");
    }
}
