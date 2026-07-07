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
        call::{
            clear_test_call_responses, install_upstream_mock,
            push_delayed_test_call_response, push_pending_test_call_response,
            push_test_call_response, test_call_response_queue_len,
        },
        failover_loop::run_failover_candidates,
        types::{BudgetAwareRouter, BudgetCandidate, CandidateSelectionMode},
    };
    use crate::{
        app::App,
        app_state::AppState,
        config::{
            credentials::ProviderCredentialId,
            provider_limits::ProviderLimitConfig, router::RouterConfig,
        },
        dispatcher::Dispatcher,
        endpoints::EndpointType,
        error::api::ApiError,
        middleware::mapper::model::ModelMapper,
        router::{
            capability::{ModelCapability, RequestRequirements},
            provider_attempt::ModelCooldownKey,
            routed_identity::REAL_MODE_MODEL_AND_PROVIDER,
        },
        tests::TestDefault,
        types::{
            extensions::{
                CallerRequestContext, PendingRouteTrace, RoutePlanContext,
                WorkUnitSource,
            },
            model_id::ModelId,
            provider::InferenceProvider,
            router::RouterId,
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

    fn llm7() -> InferenceProvider {
        InferenceProvider::Named("llm7".into())
    }

    fn vllm() -> InferenceProvider {
        InferenceProvider::Named("vllm".into())
    }

    fn named_provider(name: &str) -> InferenceProvider {
        InferenceProvider::Named(name.into())
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

    fn overload_503() -> crate::types::response::Response {
        http::Response::builder()
            .status(StatusCode::SERVICE_UNAVAILABLE)
            .body(Body::from("model is overloaded"))
            .unwrap()
    }

    fn provider_bad_request() -> crate::types::response::Response {
        http::Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .body(Body::from("model is not supported by this upstream"))
            .unwrap()
    }

    fn model_currently_unavailable() -> crate::types::response::Response {
        http::Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .body(Body::from(
                r#"{"error":{"message":"Model 'gpt-oss:20b' is currently unavailable"}}"#,
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

    fn test_router(app_state: &AppState) -> BudgetAwareRouter {
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
            model_states: Arc::new(Mutex::new(HashMap::new())),
            provider_priorities: Arc::new(IndexMap::new()),
            default_latency: Duration::from_millis(10),
            max_cooldown_wait: Duration::from_secs(0),
            selection_mode: CandidateSelectionMode::BudgetThenCapability,
            credential_round_robin: super::super::credential_balance::CredentialRoundRobin::new_shared(),
            source_model_selection:
                crate::config::router::SourceModelSelection::Strict,
        }
    }

    async fn app_state_with_route_policy(
        provider: InferenceProvider,
        slow_success_threshold_ms: Option<u64>,
        attempt_timeout_ms: Option<u64>,
    ) -> AppState {
        let mut config = crate::config::Config::test_default();
        let provider_config = config
            .provider_limits
            .providers
            .entry(provider)
            .or_insert_with(ProviderLimitConfig::default);
        provider_config.slow_success_threshold_ms = slow_success_threshold_ms;
        provider_config.attempt_timeout_ms = attempt_timeout_ms;
        App::new(config)
            .await
            .expect("test app with route policy")
            .state
    }

    fn route_memory_key(
        requirements: &RequestRequirements,
    ) -> crate::router::budget_aware::RouteMemoryKey {
        crate::router::budget_aware::RouteMemoryKey::for_route_class(
            &crate::types::router::RouterId::Named(
                "structured-output-test".into(),
            ),
            crate::endpoints::EndpointType::Chat,
            requirements,
            None,
            None,
            crate::router::budget_aware::RouteStreamMode::NonStreaming,
        )
    }

    fn route_plan_context(
        agent_name: &str,
        key: crate::router::budget_aware::RouteMemoryKey,
        candidates: Vec<BudgetCandidate>,
    ) -> RoutePlanContext {
        RoutePlanContext {
            caller: CallerRequestContext {
                agent_name: agent_name.to_string(),
                work_unit_id: Some(format!("{agent_name}-unit")),
                work_unit_source: WorkUnitSource::Explicit,
            },
            estimated_tokens: 0,
            planned_hops: u32::try_from(candidates.len()).unwrap_or(u32::MAX),
            route_memory_key: key,
            route_memory_hit: false,
            route_memory_hit_binding: None,
            full_pool: candidates,
            source_model: None,
            stream: false,
            json_schema_required: false,
            replay: None,
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
        let service = Dispatcher::new_with_model_id_without_rate_limit_events(
            app_state.clone(),
            &router_id,
            &router_config,
            provider.clone(),
            model_id.clone(),
        )
        .await
        .expect("dispatcher for mock candidate");

        BudgetCandidate {
            credential_id: ProviderCredentialId::new(format!(
                "{provider}-test"
            )),
            credential_budget_rank: 0,
            credential_cost_class: crate::config::cost_class::CostClass::Free,
            credential_tier: "free".into(),
            capability: ModelCapability {
                provider,
                model: model_id,
                context_window,
                supports_tools: true,
                supports_json_schema: true,
                supports_vision: false,
                reasoning: true,
                json_schema_rank: 0,
                intent_tier: crate::router::intent::IntentTier::Deep,
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

    fn provider_stats_row(
        app_state: &AppState,
        credential: &str,
    ) -> crate::metrics::provider::runtime::ProviderStatsRow {
        app_state
            .provider_stats_snapshot(None, Some(credential))
            .providers
            .into_iter()
            .find(|row| row.credential == credential)
            .unwrap_or_else(|| panic!("provider stats row for {credential}"))
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn structured_output_retry_repairs_before_provider_failover() {
        clear_test_call_responses();
        push_test_call_response(Ok(chat_completion("| broken | markdown |")));
        push_test_call_response(Ok(chat_completion(
            r#"{"value":"recovered_on_retry"}"#,
        )));

        let app_state = AppState::test_default().await;
        let router = test_router(&app_state);
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
            None,
        )
        .await
        .expect("second candidate should pass schema validation");

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response
                .headers()
                .get(REAL_MODE_MODEL_AND_PROVIDER)
                .and_then(|value| value.to_str().ok()),
            Some("groq-test/llama-4-scout-17b-16e-instruct")
        );

        let body = response.into_body().collect().await.unwrap().to_bytes();
        let parsed: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(
            parsed["choices"][0]["message"]["content"],
            r#"{"value":"recovered_on_retry"}"#
        );

        let groq_credential = ProviderCredentialId::new("groq-test");
        let mistral_credential = ProviderCredentialId::new("mistral-test");
        let states = router.states.lock().expect("credential states");
        assert!(
            states
                .get(&groq_credential)
                .and_then(|state| state.cooldown_until)
                .is_none(),
            "retry-repaired structured output must not poison the whole \
             credential"
        );
        assert!(
            states
                .get(&mistral_credential)
                .and_then(|state| state.cooldown_until)
                .is_none(),
            "winning credential must not stay in cooldown"
        );
        drop(states);

        let model_states = router.model_states.lock().expect("model states");
        assert!(
            model_states
                .get(&ModelCooldownKey {
                    credential_id: groq_credential,
                    model: "llama-4-scout-17b-16e-instruct".to_string(),
                })
                .and_then(|state| state.cooldown_until)
                .is_none(),
            "retry-repaired structured output must not cool down the model"
        );

        let stats = provider_stats_row(&app_state, "groq-test");
        assert_eq!(stats.calls.attempts, 2);
        assert_eq!(stats.calls.semantic_error, 1);
        assert_eq!(stats.calls.success_degraded, 1);
        assert_eq!(stats.status_codes.get("200"), Some(&2));
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn structured_output_retry_upstream_failure_records_repair_attempt() {
        clear_test_call_responses();
        push_test_call_response(Ok(chat_completion("| broken | markdown |")));
        push_test_call_response(Ok(overload_503()));
        push_test_call_response(Ok(chat_completion(
            r#"{"value":"recovered_after_retry_failure"}"#,
        )));

        let app_state = AppState::test_default().await;
        let router = test_router(&app_state);
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
            router,
            request_parts(),
            json_schema_request_body(""),
            candidates,
            RequestRequirements {
                json_schema_required: true,
                reasoning_preferred: true,
                ..RequestRequirements::default()
            },
            None,
        )
        .await
        .expect(
            "retry upstream failure should fail over to the next candidate",
        );

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response
                .headers()
                .get(REAL_MODE_MODEL_AND_PROVIDER)
                .and_then(|value| value.to_str().ok()),
            Some("mistral-test/magistral-medium-latest")
        );

        let groq_stats = provider_stats_row(&app_state, "groq-test");
        assert_eq!(groq_stats.calls.attempts, 2);
        assert_eq!(groq_stats.calls.semantic_error, 1);
        assert_eq!(groq_stats.calls.server_error, 1);
        assert_eq!(groq_stats.status_codes.get("200"), Some(&1));
        assert_eq!(groq_stats.status_codes.get("503"), Some(&1));

        let mistral_stats = provider_stats_row(&app_state, "mistral-test");
        assert_eq!(mistral_stats.calls.attempts, 1);
        assert_eq!(mistral_stats.calls.success, 1);
        assert_eq!(mistral_stats.status_codes.get("200"), Some(&1));
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn llm7_schema_conformance_reflector_repairs_invalid_json_schema() {
        clear_test_call_responses();
        push_test_call_response(Ok(chat_completion(
            "```json\n{\"wrong\":\"kept text\"}\n```",
        )));
        push_test_call_response(Ok(chat_completion(
            r#"{"value":"kept text"}"#,
        )));

        let app_state = AppState::test_default().await;
        let router = test_router(&app_state);
        let candidates =
            vec![candidate(&app_state, llm7(), "fast", Some(32_000)).await];

        let response = run_failover_candidates(
            router.clone(),
            request_parts(),
            json_schema_request_body(""),
            candidates,
            RequestRequirements {
                json_schema_required: true,
                ..RequestRequirements::default()
            },
            None,
        )
        .await
        .expect("llm7 reflector should repair the schema mismatch");

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response
                .headers()
                .get(REAL_MODE_MODEL_AND_PROVIDER)
                .and_then(|value| value.to_str().ok()),
            Some("llm7-test/fast")
        );
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let parsed: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(
            parsed["choices"][0]["message"]["content"],
            r#"{"value":"kept text"}"#
        );

        let llm7_credential = ProviderCredentialId::new("llm7-test");
        let states = router.states.lock().expect("credential states");
        assert!(
            states
                .get(&llm7_credential)
                .and_then(|state| state.cooldown_until)
                .is_none(),
            "repaired structured output must not put llm7 into cooldown"
        );
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn llm7_reflector_accepts_single_markdown_fenced_json_object() {
        clear_test_call_responses();
        push_test_call_response(Ok(chat_completion(
            r#"{"wrong":"kept text"}"#,
        )));
        push_test_call_response(Ok(chat_completion(
            "```json\n{\"value\":\"kept text\"}\n```",
        )));

        let app_state = AppState::test_default().await;
        let router = test_router(&app_state);
        let candidates =
            vec![candidate(&app_state, llm7(), "fast", Some(32_000)).await];

        let response = run_failover_candidates(
            router,
            request_parts(),
            json_schema_request_body(""),
            candidates,
            RequestRequirements {
                json_schema_required: true,
                ..RequestRequirements::default()
            },
            None,
        )
        .await
        .expect("llm7 markdown-fenced reflector JSON should normalize");

        assert_eq!(response.status(), StatusCode::OK);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let parsed: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(
            parsed["choices"][0]["message"]["content"],
            r#"{"value":"kept text"}"#
        );
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn structured_output_failover_returns_route_exhausted_when_all_invalid()
     {
        clear_test_call_responses();
        push_test_call_response(Ok(chat_completion("not json at all")));
        push_test_call_response(Ok(chat_completion("| table |")));
        push_test_call_response(Ok(chat_completion("still not json")));

        let app_state = AppState::test_default().await;
        let router = test_router(&app_state);
        let candidates = vec![
            candidate(
                &app_state,
                groq(),
                "llama-4-scout-17b-16e-instruct",
                None,
            )
            .await,
            candidate(&app_state, mistral(), "magistral-medium-latest", None)
                .await,
            candidate(&app_state, cerebras(), "openai/gpt-oss-120b", None)
                .await,
        ];

        let response = run_failover_candidates(
            router,
            request_parts(),
            json_schema_request_body(""),
            candidates,
            RequestRequirements {
                json_schema_required: true,
                ..RequestRequirements::default()
            },
            None,
        )
        .await
        .expect("all syntax-invalid outputs must return terminal response");

        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn large_context_json_schema_order_1111799_failover_succeeds_on_third()
     {
        clear_test_call_responses();
        push_test_call_response(Ok(chat_completion("```json\n{broken")));
        push_test_call_response(Ok(chat_completion("")));
        push_test_call_response(Ok(chat_completion("| still invalid |")));
        push_test_call_response(Ok(chat_completion("{\"wrong\":\"schema\"}")));
        push_test_call_response(Ok(chat_completion(
            r#"{"value":"ok for order 1111799"}"#,
        )));

        let dossier = "x".repeat(120_000);
        let app_state = AppState::test_default().await;
        let router = test_router(&app_state);
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
            None,
        )
        .await
        .expect(
            "third candidate should win after two structured-output failures",
        );

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response
                .headers()
                .get(REAL_MODE_MODEL_AND_PROVIDER)
                .and_then(|value| value.to_str().ok()),
            Some("cerebras-test/openai/gpt-oss-120b")
        );
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn replan_reaches_full_pool_after_current_plan_slot_exhaustion() {
        clear_test_call_responses();
        push_test_call_response(Ok(overload_503()));
        push_test_call_response(Ok(chat_completion("replanned fallback")));

        let app_state = AppState::test_default().await;
        let router = test_router(&app_state);
        let vllm_primary =
            candidate(&app_state, vllm(), "model-a", Some(32_000)).await;
        let vllm_same_slot =
            candidate(&app_state, vllm(), "model-b", Some(32_000)).await;
        let fallback =
            candidate(&app_state, cerebras(), "openai/gpt-oss-120b", None)
                .await;
        let full_pool = vec![
            vllm_primary.clone(),
            vllm_same_slot.clone(),
            fallback.clone(),
        ];
        let initial_plan = vec![vllm_primary, vllm_same_slot];
        let requirements = RequestRequirements::default();
        let caller = CallerRequestContext {
            agent_name: "replan-regression".to_string(),
            work_unit_id: Some("unit-replan".to_string()),
            work_unit_source: WorkUnitSource::Explicit,
        };
        let mut parts = request_parts();
        parts.extensions.insert(RoutePlanContext {
            caller,
            full_pool,
            estimated_tokens: 0,
            route_memory_key:
                crate::router::budget_aware::RouteMemoryKey::for_route_class(
                    &crate::types::router::RouterId::Named(
                        "structured-output-test".into(),
                    ),
                    crate::endpoints::EndpointType::Chat,
                    &requirements,
                    None,
                    None,
                    crate::router::budget_aware::RouteStreamMode::NonStreaming,
                ),
            route_memory_hit: false,
            route_memory_hit_binding: None,
            planned_hops: 2,
            source_model: None,
            stream: false,
            json_schema_required: false,
            replay: None,
        });

        let response = run_failover_candidates(
            router,
            parts,
            Bytes::from(r#"{"model":"openai/gpt-5-mini","messages":[]}"#),
            initial_plan,
            requirements,
            None,
        )
        .await
        .expect("replan should reach fallback outside exhausted current plan");

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response
                .headers()
                .get(REAL_MODE_MODEL_AND_PROVIDER)
                .and_then(|value| value.to_str().ok()),
            Some("cerebras-test/openai/gpt-oss-120b")
        );
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn failover_continues_after_provider_bad_request() {
        clear_test_call_responses();
        push_test_call_response(Ok(provider_bad_request()));
        push_test_call_response(Ok(chat_completion("recovered after 400")));

        let app_state = AppState::test_default().await;
        let router = test_router(&app_state);
        let candidates = vec![
            candidate(&app_state, llm7(), "model-a", Some(32_000)).await,
            candidate(&app_state, mistral(), "magistral-medium-latest", None)
                .await,
        ];

        let response = run_failover_candidates(
            router,
            request_parts(),
            Bytes::from(r#"{"model":"openai/gpt-5-mini","messages":[]}"#),
            candidates,
            RequestRequirements::default(),
            None,
        )
        .await
        .expect("provider 400 should fail over to the next candidate");

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response
                .headers()
                .get(REAL_MODE_MODEL_AND_PROVIDER)
                .and_then(|value| value.to_str().ok()),
            Some("mistral-test/magistral-medium-latest")
        );
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn llm7_currently_unavailable_model_keeps_same_slot_model_fallback() {
        clear_test_call_responses();
        push_test_call_response(Ok(model_currently_unavailable()));
        push_test_call_response(Ok(chat_completion(
            "recovered on another llm7 model",
        )));

        let app_state = AppState::test_default().await;
        let router = test_router(&app_state);
        let candidates = vec![
            candidate(&app_state, llm7(), "gpt-oss:20b", Some(32_000)).await,
            candidate(&app_state, llm7(), "fast", Some(32_000)).await,
        ];

        let response = run_failover_candidates(
            router,
            request_parts(),
            Bytes::from(r#"{"model":"openai/gpt-5-mini","messages":[]}"#),
            candidates,
            RequestRequirements::default(),
            None,
        )
        .await
        .expect("unavailable llm7 model should not kill the credential slot");

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response
                .headers()
                .get(REAL_MODE_MODEL_AND_PROVIDER)
                .and_then(|value| value.to_str().ok()),
            Some("llm7-test/fast")
        );
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn slow_success_records_degraded_stats_and_route_memory() {
        clear_test_call_responses();

        let app_state =
            app_state_with_route_policy(llm7(), Some(1), None).await;
        let router = test_router(&app_state);
        let candidate =
            candidate(&app_state, llm7(), "fast", Some(32_000)).await;
        clear_test_call_responses();
        push_delayed_test_call_response(
            Duration::from_millis(25),
            Ok(chat_completion("slow but valid")),
        );
        let requirements = RequestRequirements::default();
        let key = route_memory_key(&requirements);
        let mut parts = request_parts();
        parts.extensions.insert(RoutePlanContext {
            caller: CallerRequestContext {
                agent_name: "slow-success-regression".to_string(),
                work_unit_id: Some("unit-slow".to_string()),
                work_unit_source: WorkUnitSource::Explicit,
            },
            full_pool: vec![candidate.clone()],
            estimated_tokens: 0,
            route_memory_key: key.clone(),
            route_memory_hit: false,
            route_memory_hit_binding: None,
            planned_hops: 1,
            source_model: None,
            stream: false,
            json_schema_required: false,
            replay: None,
        });

        let response = run_failover_candidates(
            router,
            parts,
            Bytes::from(r#"{"model":"openai/gpt-5-mini","messages":[]}"#),
            vec![candidate],
            requirements,
            None,
        )
        .await
        .expect("slow success still serves the client");

        assert_eq!(response.status(), StatusCode::OK);
        let _ = response.into_body().collect().await.unwrap();

        let preferred = app_state.route_memory().preferred(&key).await;
        assert_eq!(preferred.len(), 1);
        assert_eq!(
            preferred[0].binding.credential_id,
            ProviderCredentialId::new("llm7-test")
        );
        assert_eq!(preferred[0].binding.model, "fast");
        assert_eq!(preferred[0].score, 0.25);
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn configured_attempt_timeout_falls_back_to_next_candidate() {
        clear_test_call_responses();

        let app_state =
            app_state_with_route_policy(llm7(), None, Some(1)).await;
        assert_eq!(
            app_state.config().provider_limits.attempt_timeout(
                &llm7(),
                "free",
                "slow-model",
            ),
            Some(Duration::from_millis(1))
        );
        let router = test_router(&app_state);
        assert_eq!(
            router.app_state.config().provider_limits.attempt_timeout(
                &llm7(),
                "free",
                "slow-model",
            ),
            Some(Duration::from_millis(1))
        );
        let candidates = vec![
            candidate(&app_state, llm7(), "slow-model", Some(32_000)).await,
            candidate(&app_state, mistral(), "magistral-medium-latest", None)
                .await,
        ];
        clear_test_call_responses();
        install_upstream_mock(
            gateway_tests::UpstreamMockScript::new().binding(
                "mistral-test",
                "magistral-medium-latest",
                vec![gateway_tests::upstream::ok_chat_completion],
            ),
        );
        push_pending_test_call_response();
        assert_eq!(test_call_response_queue_len(), 1);
        assert_eq!(
            router.app_state.config().provider_limits.attempt_timeout(
                &candidates[0].capability.provider,
                candidates[0].credential_tier.as_str(),
                &candidates[0].capability.model.to_string(),
            ),
            Some(Duration::from_millis(1))
        );

        let response = run_failover_candidates(
            router,
            request_parts(),
            Bytes::from(r#"{"model":"openai/gpt-5-mini","messages":[]}"#),
            candidates,
            RequestRequirements::default(),
            None,
        )
        .await
        .expect("timeout should fall back to next candidate");

        assert_eq!(test_call_response_queue_len(), 0);
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response
                .headers()
                .get(REAL_MODE_MODEL_AND_PROVIDER)
                .and_then(|value| value.to_str().ok()),
            Some("mistral-test/magistral-medium-latest")
        );
        let _ = response.into_body().collect().await.unwrap();
        assert_eq!(
            app_state.credential_health().model_success_rate(
                &llm7(),
                &ProviderCredentialId::new("llm7-test"),
                "slow-model",
            ),
            0.0
        );
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn configured_attempt_timeout_finalizes_no_status_terminal() {
        clear_test_call_responses();

        let app_state =
            app_state_with_route_policy(llm7(), None, Some(1)).await;
        assert_eq!(
            app_state.config().provider_limits.attempt_timeout(
                &llm7(),
                "free",
                "slow-model",
            ),
            Some(Duration::from_millis(1))
        );
        let router = test_router(&app_state);
        assert_eq!(
            router.app_state.config().provider_limits.attempt_timeout(
                &llm7(),
                "free",
                "slow-model",
            ),
            Some(Duration::from_millis(1))
        );
        let candidates = vec![
            candidate(&app_state, llm7(), "slow-model", Some(32_000)).await,
        ];
        clear_test_call_responses();
        push_pending_test_call_response();
        assert_eq!(test_call_response_queue_len(), 1);
        assert_eq!(
            router.app_state.config().provider_limits.attempt_timeout(
                &candidates[0].capability.provider,
                candidates[0].credential_tier.as_str(),
                &candidates[0].capability.model.to_string(),
            ),
            Some(Duration::from_millis(1))
        );

        let response = run_failover_candidates(
            router,
            request_parts(),
            Bytes::from(r#"{"model":"openai/gpt-5-mini","messages":[]}"#),
            candidates,
            RequestRequirements::default(),
            None,
        )
        .await
        .expect("terminal timeout should return a gateway response");

        assert_eq!(test_call_response_queue_len(), 0);
        assert_eq!(response.status(), StatusCode::GATEWAY_TIMEOUT);
        let pending = response
            .extensions()
            .get::<PendingRouteTrace>()
            .expect("pending route trace");
        assert_eq!(pending.outcome_label, "upstream_timeout");
        assert_eq!(pending.terminal_status, None);
        assert_eq!(pending.terminal_provider, Some(llm7()));
        assert_eq!(pending.terminal_credential.as_deref(), Some("llm7-test"));
        let _ = response.into_body().collect().await.unwrap();
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn timeout_then_http_failures_exhausts_route_without_stale_timeout_terminal()
     {
        clear_test_call_responses();

        let app_state =
            app_state_with_route_policy(llm7(), None, Some(1)).await;
        let router = test_router(&app_state);
        let candidates = vec![
            candidate(&app_state, llm7(), "slow-model", Some(32_000)).await,
            candidate(&app_state, mistral(), "magistral-medium-latest", None)
                .await,
            candidate(&app_state, cerebras(), "openai/gpt-oss-120b", None)
                .await,
        ];
        clear_test_call_responses();
        push_pending_test_call_response();
        push_test_call_response(Ok(overload_503()));
        push_test_call_response(Ok(overload_503()));

        let response = run_failover_candidates(
            router,
            request_parts(),
            Bytes::from(r#"{"model":"openai/gpt-5-mini","messages":[]}"#),
            candidates,
            RequestRequirements::default(),
            None,
        )
        .await
        .expect("HTTP failures after timeout should exhaust the route");

        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
        assert!(
            response
                .headers()
                .get(REAL_MODE_MODEL_AND_PROVIDER)
                .is_none(),
            "route exhaustion must not inherit the stale timeout candidate \
             identity"
        );
        if let Some(pending) = response.extensions().get::<PendingRouteTrace>()
        {
            assert_ne!(
                pending.outcome_label, "upstream_timeout",
                "route exhaustion must not inherit stale no-status terminal \
                 trace"
            );
        }
        let _ = response.into_body().collect().await.unwrap();
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn call_candidate_error_falls_back_with_no_status_summary() {
        clear_test_call_responses();
        push_test_call_response(Err(ApiError::Panic(
            "mock transport failure".to_string(),
        )));
        push_test_call_response(Ok(chat_completion("fallback")));

        let app_state = AppState::test_default().await;
        let router = test_router(&app_state);
        let candidates = vec![
            candidate(&app_state, llm7(), "fast", Some(32_000)).await,
            candidate(&app_state, mistral(), "magistral-medium-latest", None)
                .await,
        ];

        let response = run_failover_candidates(
            router,
            request_parts(),
            Bytes::from(r#"{"model":"openai/gpt-5-mini","messages":[]}"#),
            candidates,
            RequestRequirements::default(),
            None,
        )
        .await
        .expect("transport error should fall back to next candidate");

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response
                .headers()
                .get(REAL_MODE_MODEL_AND_PROVIDER)
                .and_then(|value| value.to_str().ok()),
            Some("mistral-test/magistral-medium-latest")
        );
        let pending = response
            .extensions()
            .get::<PendingRouteTrace>()
            .cloned()
            .expect("pending route trace");
        assert_eq!(pending.outcome_label, "success");
        assert_eq!(pending.terminal_status, Some(200));
        assert_eq!(pending.summary.failed_attempts_total, 1);
        assert_eq!(pending.summary.failover_count, 1);
        assert_eq!(pending.summary.attempt_statuses, "0");
        assert_eq!(
            pending.summary.attempt_error_classes,
            "upstream_incomplete"
        );
        assert_eq!(
            pending.summary.last_failed_credential.as_deref(),
            Some("llm7-test")
        );
        assert_eq!(pending.summary.last_failed_model.as_deref(), Some("fast"));
        let _ = response.into_body().collect().await.unwrap();
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn route_memory_penalty_is_absent_when_binding_was_not_recorded() {
        clear_test_call_responses();
        push_test_call_response(Ok(overload_503()));
        push_test_call_response(Ok(chat_completion("fallback")));

        let app_state = AppState::test_default().await;
        let router = test_router(&app_state);
        let candidates = vec![
            candidate(&app_state, llm7(), "fast", Some(32_000)).await,
            candidate(&app_state, mistral(), "magistral-medium-latest", None)
                .await,
        ];
        let requirements = RequestRequirements::default();
        let key = route_memory_key(&requirements);
        let mut parts = request_parts();
        parts.extensions.insert(route_plan_context(
            "route-memory-penalty-regression",
            key,
            candidates.clone(),
        ));

        let response = run_failover_candidates(
            router,
            parts,
            Bytes::from(r#"{"model":"openai/gpt-5-mini","messages":[]}"#),
            candidates,
            requirements,
            None,
        )
        .await
        .expect("fallback should still serve the client");

        assert_eq!(response.status(), StatusCode::OK);
        let pending = response
            .extensions()
            .get::<PendingRouteTrace>()
            .cloned()
            .expect("pending route trace");
        assert_eq!(pending.summary.failed_attempts_total, 1);
        assert_eq!(pending.summary.failover_count, 1);
        assert!(
            pending.summary.route_memory_penalized_binding.is_none(),
            "trace must not claim a route-memory penalty when registry had no \
             binding"
        );
        assert_eq!(pending.route_memory_invalidated, Some(false));
        assert_eq!(
            pending.summary.route_memory_recorded_binding.as_deref(),
            Some("mistral-test/magistral-medium-latest")
        );
        assert_eq!(pending.summary.route_memory_policy, "full");
        let _ = response.into_body().collect().await.unwrap();
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn terminal_semantic_failure_does_not_increment_failover_count() {
        clear_test_call_responses();
        push_test_call_response(Ok(chat_completion("not json at all")));

        let app_state = AppState::test_default().await;
        let router = test_router(&app_state);
        let candidates =
            vec![candidate(&app_state, llm7(), "fast", Some(32_000)).await];

        let response = run_failover_candidates(
            router,
            request_parts(),
            json_schema_request_body(""),
            candidates,
            RequestRequirements {
                json_schema_required: true,
                ..RequestRequirements::default()
            },
            None,
        )
        .await
        .expect("terminal semantic failure should return gateway response");

        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
        let pending = response
            .extensions()
            .get::<PendingRouteTrace>()
            .cloned()
            .expect("pending route trace");
        assert_eq!(pending.summary.failed_attempts_total, 1);
        assert_eq!(pending.summary.failover_count, 0);
        assert_eq!(pending.summary.attempt_statuses, "200");
        assert_eq!(
            pending.summary.attempt_error_classes,
            "invalid_structured_json"
        );
        let _ = response.into_body().collect().await.unwrap();
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn failover_walks_every_planned_hop_before_route_exhaustion() {
        let provider_names = [
            "groq",
            "mistral",
            "cerebras",
            "llm7",
            "vllm",
            "longcat",
            "hyperbolic",
            "opencode",
            "sambanova",
        ];

        clear_test_call_responses();
        for _ in 0..provider_names.len() {
            push_test_call_response(Ok(overload_503()));
        }

        let app_state = AppState::test_default().await;
        let router = test_router(&app_state);
        let mut plan = Vec::new();
        for provider in provider_names {
            plan.push(
                candidate(
                    &app_state,
                    named_provider(provider),
                    "model-a",
                    Some(32_000),
                )
                .await,
            );
        }

        let response = run_failover_candidates(
            router,
            request_parts(),
            Bytes::from(r#"{"model":"openai/gpt-5-mini","messages":[]}"#),
            plan,
            RequestRequirements::default(),
            None,
        )
        .await
        .expect("all failoverable hops should exhaust the route");

        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
        for provider in provider_names {
            let credential = format!("{provider}-test");
            let credential_id = ProviderCredentialId::new(credential.as_str());
            let provider = named_provider(provider);
            assert!(
                app_state
                    .credential_health()
                    .model_latency_ms(&provider, &credential_id, "model-a")
                    .is_some(),
                "expected a recorded model attempt for {credential}"
            );
            assert_eq!(
                app_state.credential_health().model_success_rate(
                    &provider,
                    &credential_id,
                    "model-a",
                ),
                0.0,
                "expected failed model health for {credential}"
            );
        }
    }
}
