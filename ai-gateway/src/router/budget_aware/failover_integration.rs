#[cfg(all(test, feature = "testing"))]
mod structured_output_failover {
    use std::{
        collections::HashMap,
        sync::{Arc, Mutex},
        time::Duration,
    };

    use axum_core::body::Body;
    use bytes::Bytes;
    use http::{Request, StatusCode};
    use http_body_util::BodyExt;
    use indexmap::IndexMap;

    use super::super::{
        call::{clear_test_call_responses, push_test_call_response},
        failover_loop::run_failover_candidates,
        types::{BudgetAwareRouter, BudgetCandidate, CandidateSelectionMode},
    };
    use crate::{
        app_state::AppState,
        config::router::RouterConfig,
        dispatcher::Dispatcher,
        endpoints::EndpointType,
        error::{api::ApiError, internal::InternalError},
        middleware::mapper::model::ModelMapper,
        router::{
            capability::{ModelCapability, RequestRequirements},
            routed_identity::REAL_MODE_MODEL_AND_PROVIDER,
        },
        types::{
            model_id::ModelId, provider::InferenceProvider, router::RouterId,
        },
    };

    fn groq() -> InferenceProvider {
        InferenceProvider::Named("groq".into())
    }

    fn mistral() -> InferenceProvider {
        InferenceProvider::Named("mistral".into())
    }

    fn cerebras() -> InferenceProvider {
        InferenceProvider::Named("cerebras".into())
    }

    fn chat_completion(content: &str) -> crate::types::response::Response {
        http::Response::builder()
            .status(StatusCode::OK)
            .header(http::header::CONTENT_TYPE, "application/json")
            .body(Body::from(
                serde_json::json!({
                    "choices": [{"message": {"content": content}}]
                })
                .to_string(),
            ))
            .unwrap()
    }

    fn json_schema_request_body(extra_user_content: &str) -> Bytes {
        Bytes::from(
            serde_json::json!({
                "model": "openai/gpt-5-mini",
                "stream": false,
                "response_format": {
                    "type": "json_schema",
                    "json_schema": {
                        "name": "sales_qa_dossier",
                        "strict": true,
                        "schema": {
                            "type": "object",
                            "properties": {
                                "value": {"type": "string"}
                            },
                            "required": ["value"],
                            "additionalProperties": false
                        }
                    }
                },
                "messages": [{
                    "role": "user",
                    "content": format!("order 1111799 dossier {extra_user_content}")
                }]
            })
            .to_string(),
        )
    }

    async fn test_router(app_state: AppState) -> BudgetAwareRouter {
        let router_config = Arc::new(RouterConfig::default());
        BudgetAwareRouter {
            app_state: app_state.clone(),
            router_id: RouterId::Named("structured-output-test".into()),
            endpoint_type: EndpointType::Chat,
            strategy: "budget-aware-capability-after",
            candidates: Arc::new(vec![]),
            model_mapper: ModelMapper::new_for_router(
                app_state.clone(),
                router_config,
            ),
            states: Arc::new(Mutex::new(HashMap::new())),
            provider_priorities: Arc::new(IndexMap::new()),
            default_latency: Duration::from_millis(10),
            max_cooldown_wait: Duration::from_secs(0),
            selection_mode: CandidateSelectionMode::BudgetThenCapability,
        }
    }

    async fn candidate(
        app_state: &AppState,
        provider: InferenceProvider,
        model: &str,
        context_window: Option<u32>,
    ) -> BudgetCandidate {
        let router_id = RouterId::Named("structured-output-test".into());
        let router_config = Arc::new(RouterConfig::default());
        let model_id =
            ModelId::from_str_and_provider(provider.clone(), model).unwrap();
        let service =
            Dispatcher::new_with_model_id_without_rate_limit_events(
                app_state.clone(),
                &router_id,
                &router_config,
                provider.clone(),
                model_id.clone(),
            )
            .await
            .expect("dispatcher for mock candidate");

        BudgetCandidate {
            capability: ModelCapability {
                provider,
                model: model_id,
                context_window,
                supports_tools: true,
                supports_json_schema: true,
                supports_vision: false,
                reasoning: true,
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

    #[tokio::test]
    #[serial_test::serial]
    async fn structured_output_failover_succeeds_on_second_candidate() {
        clear_test_call_responses();
        push_test_call_response(Ok(chat_completion("| broken | markdown |")));
        push_test_call_response(Ok(chat_completion(
            r#"{"unexpected_field":"wrong_shape_only_syntax_ok"}"#,
        )));

        let app_state = AppState::test_default().await;
        let router = test_router(app_state.clone()).await;
        let candidates = vec![
            candidate(
                &app_state,
                groq(),
                "llama-4-scout-17b-16e-instruct",
                Some(131_072),
            )
            .await,
            candidate(
                &app_state,
                mistral(),
                "magistral-medium-latest",
                Some(131_072),
            )
            .await,
        ];

        let response = run_failover_candidates(
            router.clone(),
            request_parts(),
            json_schema_request_body(""),
            candidates,
            RequestRequirements {
                json_schema_required: true,
                reasoning_preferred: true,
                ..RequestRequirements::default()
            },
        )
        .await
        .expect("second candidate should pass syntax gate");

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response
                .headers()
                .get(REAL_MODE_MODEL_AND_PROVIDER)
                .and_then(|value| value.to_str().ok()),
            Some("mistral/magistral-medium-latest")
        );

        let body = response.into_body().collect().await.unwrap().to_bytes();
        let parsed: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(
            parsed["choices"][0]["message"]["content"],
            r#"{"unexpected_field":"wrong_shape_only_syntax_ok"}"#
        );

        let states = router.states.lock().expect("provider states");
        assert!(
            states.get(&groq()).and_then(|state| state.cooldown_until).is_some(),
            "first provider must be marked faulty after invalid structured output"
        );
        assert!(
            states
                .get(&mistral())
                .and_then(|state| state.cooldown_until)
                .is_none(),
            "winning provider must not stay in cooldown"
        );
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn structured_output_failover_returns_provider_not_found_when_all_invalid(
    ) {
        clear_test_call_responses();
        push_test_call_response(Ok(chat_completion("not json at all")));
        push_test_call_response(Ok(chat_completion("| table |")));
        push_test_call_response(Ok(chat_completion("still not json")));

        let app_state = AppState::test_default().await;
        let router = test_router(app_state.clone()).await;
        let candidates = vec![
            candidate(&app_state, groq(), "llama-4-scout-17b-16e-instruct", None)
                .await,
            candidate(&app_state, mistral(), "magistral-medium-latest", None)
                .await,
            candidate(
                &app_state,
                cerebras(),
                "openai/gpt-oss-120b",
                None,
            )
            .await,
        ];

        let error = run_failover_candidates(
            router,
            request_parts(),
            json_schema_request_body(""),
            candidates,
            RequestRequirements {
                json_schema_required: true,
                ..RequestRequirements::default()
            },
        )
        .await
        .expect_err("all syntax-invalid outputs must not return HTTP 200");

        assert!(matches!(
            error,
            ApiError::Internal(InternalError::ProviderNotFound)
        ));
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn large_context_json_schema_order_1111799_failover_succeeds_on_third(
    ) {
        clear_test_call_responses();
        push_test_call_response(Ok(chat_completion("```json\n{broken")));
        push_test_call_response(Ok(chat_completion("")));
        push_test_call_response(Ok(chat_completion(
            r#"{"value":"ok for order 1111799"}"#,
        )));

        let dossier = "x".repeat(120_000);
        let app_state = AppState::test_default().await;
        let router = test_router(app_state.clone()).await;
        let candidates = vec![
            candidate(
                &app_state,
                groq(),
                "llama-4-scout-17b-16e-instruct",
                Some(131_072),
            )
            .await,
            candidate(
                &app_state,
                mistral(),
                "magistral-medium-latest",
                Some(200_000),
            )
            .await,
            candidate(
                &app_state,
                cerebras(),
                "openai/gpt-oss-120b",
                Some(131_072),
            )
            .await,
        ];

        let response = run_failover_candidates(
            router,
            request_parts(),
            json_schema_request_body(&dossier),
            candidates,
            RequestRequirements {
                json_schema_required: true,
                min_context_tokens: Some(62_000),
                reasoning_preferred: true,
                ..RequestRequirements::default()
            },
        )
        .await
        .expect("third candidate should win after two structured-output failures");

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response
                .headers()
                .get(REAL_MODE_MODEL_AND_PROVIDER)
                .and_then(|value| value.to_str().ok()),
            Some("cerebras/openai/gpt-oss-120b")
        );
    }
}
