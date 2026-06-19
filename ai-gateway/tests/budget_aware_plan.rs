use std::collections::HashSet;

use ai_gateway::{
    app_state::AppState,
    tests::budget_aware::{
        CallOutcome, CredentialHealthRegistry, InferenceProvider,
        MAX_PLAN_HOPS, RequestRequirements, empty_router,
        gemini_model_candidate, hash_bias, openrouter_model_candidate,
        plan_route_chain,
    },
    types::extensions::{CallerRequestContext, WorkUnitSource},
};

#[tokio::test]
async fn plan_respects_max_hops() {
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
    )
    .await;
    assert!(plan.chain.len() <= MAX_PLAN_HOPS);
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
    )
    .await;
    assert!(plan.chain.is_empty());
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

#[test]
fn hash_bias_differs_across_work_units() {
    let a = hash_bias("invoker", "unit-alpha", "gemini-free-9");
    let b = hash_bias("invoker", "unit-beta", "gemini-free-9");
    assert_ne!(a, b);
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
    )
    .await;
    assert!(
        plan.chain
            .iter()
            .all(|c| c.credential_id.as_str() == "gemini-free-9")
    );
}
