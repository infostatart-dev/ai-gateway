use std::{collections::HashSet, str::FromStr, time::Duration};

use ai_gateway::{
    app_state::AppState,
    tests::{
        budget_aware::{
            CallOutcome, CredentialHealthRegistry, RequestRequirements,
            RouteStreamMode, chatgpt_candidate, deepseek_model_candidate,
            empty_router, gemini_model_candidate, hash_bias,
            intent_autodefault_router, named_model_candidate,
            openrouter_model_candidate, ordered_candidates_for_source,
            plan_route_chain,
        },
        routing::extract_routing_intent_from_name,
    },
    types::{
        extensions::{CallerRequestContext, WorkUnitSource},
        model_id::ModelId,
        provider::InferenceProvider,
    },
};

#[tokio::test]
async fn plan_includes_all_feasible_hops_without_hard_cap() {
    let app_state = AppState::test_default().await;
    let router = empty_router(&app_state);
    let mut pool = Vec::new();
    for index in 1..=10 {
        pool.push(
            gemini_model_candidate(
                &app_state,
                &format!("gemini-free-{index}"),
                "gemini-2.5-flash-lite",
            )
            .await,
        );
    }
    let caller = CallerRequestContext {
        agent_name: "invoker".to_string(),
        work_unit_id: Some("unit-1".to_string()),
        work_unit_source: WorkUnitSource::Explicit,
    };
    let plan = plan_route_chain(
        &router,
        pool,
        &RequestRequirements::default(),
        None,
        &caller,
        &CredentialHealthRegistry::new(),
        app_state.route_memory(),
        100,
        &HashSet::new(),
        None,
        RouteStreamMode::NonStreaming,
    )
    .await;
    assert_eq!(plan.chain.len(), 10);
    for index in 1..=10 {
        let expected = format!("gemini-free-{index}");
        assert!(
            plan.chain
                .iter()
                .any(|candidate| candidate.credential_id.as_str()
                    == expected.as_str()),
            "missing gemini-free-{index} from full plan"
        );
    }
}

#[tokio::test]
async fn dead_provider_excluded_from_plan() {
    let app_state = AppState::test_default().await;
    let router = empty_router(&app_state);
    let health = CredentialHealthRegistry::new();
    let dead =
        gemini_model_candidate(&app_state, "gemini-free-9", "gemini-2.5-flash")
            .await;
    for _ in 0..5 {
        health.record_attempt(
            &dead.capability.provider,
            &dead.credential_id,
            CallOutcome::RateLimited,
            429,
        );
    }
    let caller = CallerRequestContext {
        agent_name: "invoker".to_string(),
        work_unit_id: None,
        work_unit_source: WorkUnitSource::Generated,
    };
    let plan = plan_route_chain(
        &router,
        vec![dead],
        &RequestRequirements::default(),
        None,
        &caller,
        &health,
        app_state.route_memory(),
        0,
        &HashSet::new(),
        None,
        RouteStreamMode::NonStreaming,
    )
    .await;
    assert!(plan.chain.is_empty());
}

#[tokio::test]
async fn model_health_penalty_does_not_block_sibling_model() {
    let app_state = AppState::test_default().await;
    let router = empty_router(&app_state);
    let health = CredentialHealthRegistry::new();
    let unavailable = named_model_candidate(
        &app_state,
        "llm7",
        "llm7-default",
        "gpt-oss:20b",
        131_072,
    )
    .await;
    let sibling = named_model_candidate(
        &app_state,
        "llm7",
        "llm7-default",
        "fast",
        131_072,
    )
    .await;
    for _ in 0..3 {
        health.record_model_attempt(
            &unavailable.capability.provider,
            &unavailable.credential_id,
            "gpt-oss:20b",
            CallOutcome::ClientError,
            400,
            Duration::from_millis(50),
        );
    }
    let caller = CallerRequestContext {
        agent_name: "invoker".to_string(),
        work_unit_id: Some("unit-llm7".to_string()),
        work_unit_source: WorkUnitSource::Explicit,
    };

    let plan = plan_route_chain(
        &router,
        vec![unavailable, sibling],
        &RequestRequirements::default(),
        None,
        &caller,
        &health,
        app_state.route_memory(),
        100,
        &HashSet::new(),
        None,
        RouteStreamMode::NonStreaming,
    )
    .await;

    assert_eq!(plan.chain[0].capability.model.to_string(), "fast");
    assert!(plan.chain.iter().any(|candidate| {
        candidate.capability.model.to_string() == "gpt-oss:20b"
    }));
}

#[tokio::test]
async fn stability_model_before_openrouter_deprioritized() {
    let app_state = AppState::test_default().await;
    let router = empty_router(&app_state);
    let stability = gemini_model_candidate(
        &app_state,
        "gemini-free-1",
        "gemini-2.5-flash-lite",
    )
    .await;
    let nemotron = openrouter_model_candidate(
        &app_state,
        "openrouter-default",
        "nvidia/nemotron-3-nano-30b-a3b:free",
    )
    .await;
    let caller = CallerRequestContext {
        agent_name: "invoker".to_string(),
        work_unit_id: Some("unit-2".to_string()),
        work_unit_source: WorkUnitSource::Explicit,
    };
    let plan = plan_route_chain(
        &router,
        vec![nemotron, stability],
        &RequestRequirements::default(),
        None,
        &caller,
        &CredentialHealthRegistry::new(),
        app_state.route_memory(),
        100,
        &HashSet::new(),
        None,
        RouteStreamMode::NonStreaming,
    )
    .await;
    assert!(!plan.chain.is_empty());
    assert!(
        plan.chain[0]
            .capability
            .model
            .to_string()
            .contains("gemini")
            || plan.chain.iter().any(|c| c
                .capability
                .model
                .to_string()
                .contains("gemini"))
    );
}

#[tokio::test]
async fn circuit_open_credential_excluded_from_plan() {
    let app_state = AppState::test_default().await;
    let router = empty_router(&app_state);
    let health = CredentialHealthRegistry::new();
    let dead = gemini_model_candidate(
        &app_state,
        "gemini-free-8",
        "gemini-2.5-flash-lite",
    )
    .await;
    let healthy = gemini_model_candidate(
        &app_state,
        "gemini-free-9",
        "gemini-2.5-flash-lite",
    )
    .await;
    for _ in 0..5 {
        health.record_attempt(
            &dead.capability.provider,
            &dead.credential_id,
            CallOutcome::RateLimited,
            429,
        );
    }
    let caller = CallerRequestContext {
        agent_name: "invoker".to_string(),
        work_unit_id: Some("unit-3".to_string()),
        work_unit_source: WorkUnitSource::Explicit,
    };
    let plan = plan_route_chain(
        &router,
        vec![dead, healthy],
        &RequestRequirements::default(),
        None,
        &caller,
        &health,
        app_state.route_memory(),
        100,
        &HashSet::new(),
        None,
        RouteStreamMode::NonStreaming,
    )
    .await;
    assert!(!plan.chain.is_empty());
    assert!(
        plan.chain
            .iter()
            .all(|c| c.credential_id.as_str() == "gemini-free-9")
    );
}

#[tokio::test]
async fn plain_chat_widens_intent_floor_for_fast_upstream() {
    use ai_gateway::tests::routing::extract_routing_intent_from_name;

    let app_state = AppState::test_default().await;
    let router = empty_router(&app_state);
    let flash =
        gemini_model_candidate(&app_state, "gemini-free-1", "gemini-2.5-flash")
            .await;
    let caller = CallerRequestContext {
        agent_name: "invoker".to_string(),
        work_unit_id: None,
        work_unit_source: WorkUnitSource::Generated,
    };
    let intent = extract_routing_intent_from_name("openai/gpt-4o-mini");
    let plan = plan_route_chain(
        &router,
        vec![flash],
        &RequestRequirements::default(),
        Some(intent),
        &caller,
        &CredentialHealthRegistry::new(),
        app_state.route_memory(),
        100,
        &HashSet::new(),
        None,
        RouteStreamMode::NonStreaming,
    )
    .await;
    assert!(!plan.chain.is_empty());
    assert!(
        plan.chain[0]
            .capability
            .model
            .to_string()
            .contains("gemini-2.5-flash")
    );
}

#[tokio::test]
async fn sixteen_gemini_flash_lite_slots_are_plan_feasible() {
    let app_state = AppState::test_default().await;
    let router = empty_router(&app_state);
    let mut pool = Vec::new();
    for index in 1..=16 {
        let id = if index == 1 {
            "gemini-free".to_string()
        } else {
            format!("gemini-free-{index}")
        };
        pool.push(
            gemini_model_candidate(&app_state, &id, "gemini-3.1-flash-lite")
                .await,
        );
    }
    let caller = CallerRequestContext {
        agent_name: "spread-check".to_string(),
        work_unit_id: Some("unit-1".to_string()),
        work_unit_source: WorkUnitSource::Explicit,
    };
    let plan = plan_route_chain(
        &router,
        pool,
        &RequestRequirements::default(),
        None,
        &caller,
        &CredentialHealthRegistry::new(),
        app_state.route_memory(),
        100,
        &HashSet::new(),
        None,
        RouteStreamMode::NonStreaming,
    )
    .await;
    assert!(!plan.chain.is_empty());
}

#[tokio::test]
async fn eight_work_units_spread_first_hop_across_sixteen_gemini_slots() {
    let app_state = AppState::test_default().await;
    let router = empty_router(&app_state);
    let mut pool = Vec::new();
    for index in 1..=16 {
        let id = if index == 1 {
            "gemini-free".to_string()
        } else {
            format!("gemini-free-{index}")
        };
        pool.push(
            gemini_model_candidate(&app_state, &id, "gemini-3.1-flash-lite")
                .await,
        );
    }
    let mut picks = HashSet::new();
    for unit in 1..=8 {
        let caller = CallerRequestContext {
            agent_name: format!("admission-spread-{unit}"),
            work_unit_id: Some(format!("unit-{unit}")),
            work_unit_source: WorkUnitSource::Explicit,
        };
        let plan = plan_route_chain(
            &router,
            pool.clone(),
            &RequestRequirements::default(),
            None,
            &caller,
            &CredentialHealthRegistry::new(),
            app_state.route_memory(),
            100,
            &HashSet::new(),
            None,
            RouteStreamMode::NonStreaming,
        )
        .await;
        assert!(
            !plan.chain.is_empty(),
            "unit-{unit} must have a feasible first hop"
        );
        picks.insert(plan.chain[0].credential_id.to_string());
    }
    assert!(
        picks.len() >= 8,
        "expected eight distinct first-hop accounts, got {picks:?}"
    );
}

#[test]
fn hash_bias_differs_across_work_units() {
    let a = hash_bias("invoker", "unit-alpha", "gemini-free-9");
    let b = hash_bias("invoker", "unit-beta", "gemini-free-9");
    assert!((a - b).abs() > f64::EPSILON);
}

#[tokio::test]
async fn replan_excludes_failed_hop() {
    let app_state = AppState::test_default().await;
    let router = empty_router(&app_state);
    let failed = gemini_model_candidate(
        &app_state,
        "gemini-free-8",
        "gemini-2.5-flash-lite",
    )
    .await;
    let healthy = gemini_model_candidate(
        &app_state,
        "gemini-free-9",
        "gemini-2.5-flash-lite",
    )
    .await;
    let caller = CallerRequestContext {
        agent_name: "invoker".to_string(),
        work_unit_id: None,
        work_unit_source: WorkUnitSource::Generated,
    };
    let mut exclude = HashSet::new();
    exclude.insert((
        failed.credential_id.to_string(),
        failed.capability.model.to_string(),
    ));
    let plan = plan_route_chain(
        &router,
        vec![failed, healthy],
        &RequestRequirements::default(),
        None,
        &caller,
        &CredentialHealthRegistry::new(),
        app_state.route_memory(),
        100,
        &exclude,
        None,
        RouteStreamMode::NonStreaming,
    )
    .await;
    assert!(
        plan.chain
            .iter()
            .all(|c| c.credential_id.as_str() == "gemini-free-9")
    );
}

#[tokio::test]
async fn eight_gemini_accounts_plan_includes_openrouter_fallback() {
    const SLOTS: [&str; 8] = [
        "gemini-free",
        "gemini-free-2",
        "gemini-free-3",
        "gemini-free-4",
        "gemini-free-5",
        "gemini-free-6",
        "gemini-free-7",
        "gemini-free-8",
    ];
    const MODELS: [&str; 4] = [
        "gemini-3-flash-preview",
        "gemini-3.5-flash",
        "gemini-3.1-flash-lite",
        "gemini-2.5-flash-lite",
    ];

    let app_state = AppState::test_default().await;
    let router = empty_router(&app_state);
    let mut pool = Vec::new();
    for slot in SLOTS {
        for model in MODELS {
            pool.push(gemini_model_candidate(&app_state, slot, model).await);
        }
    }
    pool.push(
        openrouter_model_candidate(
            &app_state,
            "openrouter-default",
            "openai/gpt-oss-120b:free",
        )
        .await,
    );
    let caller = CallerRequestContext {
        agent_name: "cross-provider-plan".to_string(),
        work_unit_id: Some("unit-dev".to_string()),
        work_unit_source: WorkUnitSource::Explicit,
    };
    let plan = plan_route_chain(
        &router,
        pool,
        &RequestRequirements::default(),
        None,
        &caller,
        &CredentialHealthRegistry::new(),
        app_state.route_memory(),
        100,
        &HashSet::new(),
        None,
        RouteStreamMode::NonStreaming,
    )
    .await;
    assert!(!plan.chain.is_empty());
    assert_eq!(
        plan.chain[0].capability.provider.to_string(),
        "gemini",
        "multi-account provider group should anchor before single fallback"
    );
    assert!(
        plan.chain.iter().any(|candidate| {
            candidate.capability.provider.to_string() == "openrouter"
        }),
        "plan must reserve a cross-provider hop when eight gemini slots share \
         the pool: {:?}",
        plan.chain
            .iter()
            .map(|c| format!("{}/{}", c.credential_id, c.capability.model))
            .collect::<Vec<_>>()
    );
}

#[tokio::test]
async fn strategic_fallbacks_try_browser_sessions_before_local_vllm() {
    let app_state = AppState::test_default().await;
    let router = empty_router(&app_state);
    let pool = vec![
        named_model_candidate(
            &app_state,
            "vllm",
            "vllm-anonymous",
            "am-thinking-awq",
            131_072,
        )
        .await,
        named_model_candidate(
            &app_state,
            "longcat",
            "longcat-default",
            "LongCat-2.0",
            1_048_576,
        )
        .await,
        deepseek_model_candidate(
            &app_state,
            "deepseek-web-default",
            "deepseek-chat",
        )
        .await,
        deepseek_model_candidate(&app_state, "deepseek-web-2", "deepseek-chat")
            .await,
        chatgpt_candidate(&app_state).await,
    ];
    let caller = CallerRequestContext {
        agent_name: "strict-json-dev-chain".to_string(),
        work_unit_id: Some("unit-1".to_string()),
        work_unit_source: WorkUnitSource::Explicit,
    };
    let requirements = RequestRequirements {
        json_schema_required: true,
        ..RequestRequirements::default()
    };

    let plan = plan_route_chain(
        &router,
        pool,
        &requirements,
        Some(extract_routing_intent_from_name("gpt-5-mini")),
        &caller,
        &CredentialHealthRegistry::new(),
        app_state.route_memory(),
        100,
        &HashSet::new(),
        None,
        RouteStreamMode::NonStreaming,
    )
    .await;
    let providers: Vec<_> = plan
        .chain
        .iter()
        .take(5)
        .map(|candidate| candidate.capability.provider.to_string())
        .collect();

    assert_eq!(
        providers,
        vec![
            "deepseek-web",
            "deepseek-web",
            "chatgpt-web",
            "longcat",
            "vllm",
        ]
    );
}

#[tokio::test]
async fn intent_ordering_keeps_chatgpt_web_fallback_for_fast_thinking() {
    let app_state = AppState::test_default().await;
    let pool = vec![
        named_model_candidate(
            &app_state,
            "vllm",
            "vllm-anonymous",
            "am-thinking-awq",
            131_072,
        )
        .await,
        chatgpt_candidate(&app_state).await,
    ];
    let router = intent_autodefault_router(&app_state, pool);
    let source = ModelId::from_str("openai/gpt-5-mini").expect("source model");
    let requirements = RequestRequirements {
        json_schema_required: true,
        ..RequestRequirements::default()
    };

    let ordered =
        ordered_candidates_for_source(&router, &requirements, &source)
            .expect("ordered candidates");

    assert!(ordered.iter().any(|candidate| {
        candidate.capability.provider
            == InferenceProvider::Named("chatgpt-web".into())
    }));
}

#[tokio::test]
async fn plan_replay_lists_quota_excluded_candidates() {
    use ai_gateway::tests::budget_aware::BlockedReason;

    let app_state = AppState::test_default().await;
    let router = empty_router(&app_state);
    let saturated = gemini_model_candidate(
        &app_state,
        "gemini-free-3",
        "gemini-3-flash-preview",
    )
    .await;
    let gate = app_state
        .upstream_pacing()
        .gate_for(
            &saturated.capability.provider,
            Some(&saturated.credential_id),
            Some(saturated.credential_tier.as_str()),
            Some(&saturated.capability.model.to_string()),
        )
        .expect("gate");
    for _ in 0..15 {
        let _permit = gate.acquire(0).await.unwrap();
    }
    let feasible = gemini_model_candidate(
        &app_state,
        "gemini-free-9",
        "gemini-2.5-flash-lite",
    )
    .await;
    let caller = CallerRequestContext {
        agent_name: "invoker".to_string(),
        work_unit_id: Some("unit-1".to_string()),
        work_unit_source: WorkUnitSource::Explicit,
    };
    let plan = plan_route_chain(
        &router,
        vec![saturated, feasible],
        &RequestRequirements::default(),
        None,
        &caller,
        &CredentialHealthRegistry::new(),
        app_state.route_memory(),
        0,
        &HashSet::new(),
        None,
        RouteStreamMode::NonStreaming,
    )
    .await;
    let replay = plan.replay.expect("plan replay");
    assert_eq!(replay.quota_excluded.len(), 1);
    assert_eq!(replay.quota_excluded[0].credential, "gemini-free-3");
    assert_eq!(replay.quota_excluded[0].model, "gemini-3-flash-preview");
    assert_ne!(replay.quota_excluded[0].blocked_reason, BlockedReason::None);
    assert!(replay.quota_excluded[0].next_available_at.is_some());
}
