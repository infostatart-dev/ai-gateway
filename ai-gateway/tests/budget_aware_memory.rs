use std::collections::HashSet;

use ai_gateway::{
    app_state::AppState,
    tests::budget_aware::{
        CredentialHealthRegistry, ProviderCredentialId, RequestRequirements,
        RouteBinding, empty_router, gemini_model_candidate, plan_route_chain,
    },
    types::extensions::{CallerRequestContext, WorkUnitSource},
};

#[tokio::test]
async fn remembered_binding_is_first_hop_when_viable() {
    let app_state = AppState::test_default().await;
    let router = empty_router(&app_state);
    let candidate = gemini_model_candidate(
        &app_state,
        "gemini-free-9",
        "gemini-3.1-flash-lite",
    )
    .await;
    let memory = app_state.route_memory();
    memory
        .record(
            "invoker",
            "unit-47",
            RouteBinding {
                credential_id: ProviderCredentialId::new("gemini-free-9"),
                model: "gemini-3.1-flash-lite".to_string(),
            },
        )
        .await;
    let caller = CallerRequestContext {
        agent_name: "invoker".to_string(),
        work_unit_id: Some("unit-47".to_string()),
        work_unit_source: WorkUnitSource::Explicit,
    };
    let plan = plan_route_chain(
        &router,
        vec![candidate],
        &RequestRequirements::default(),
        None,
        &caller,
        &CredentialHealthRegistry::new(),
        memory,
        100,
        &HashSet::new(),
    )
    .await;
    assert!(plan.route_memory_hit);
    assert_eq!(plan.chain[0].credential_id.as_str(), "gemini-free-9");
}

#[tokio::test]
async fn absent_work_unit_skips_memory() {
    let app_state = AppState::test_default().await;
    let memory = app_state.route_memory();
    memory
        .record(
            "invoker",
            "unit-47",
            RouteBinding {
                credential_id: ProviderCredentialId::new("gemini-free-9"),
                model: "gemini-3.1-flash-lite".to_string(),
            },
        )
        .await;
    let caller = CallerRequestContext {
        agent_name: "invoker".to_string(),
        work_unit_id: None,
        work_unit_source: WorkUnitSource::Generated,
    };
    let router = empty_router(&app_state);
    let candidate = gemini_model_candidate(
        &app_state,
        "gemini-free-9",
        "gemini-3.1-flash-lite",
    )
    .await;
    let plan = plan_route_chain(
        &router,
        vec![candidate],
        &RequestRequirements::default(),
        None,
        &caller,
        &CredentialHealthRegistry::new(),
        memory,
        100,
        &HashSet::new(),
    )
    .await;
    assert!(!plan.route_memory_hit);
}
