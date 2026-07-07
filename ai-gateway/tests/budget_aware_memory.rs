use std::collections::HashSet;

use ai_gateway::{
    app_state::AppState,
    endpoints::EndpointType,
    tests::budget_aware::{
        CredentialHealthRegistry, ProviderCredentialId, RequestRequirements,
        RouteBinding, RouteMemoryKey, RouteStreamMode, empty_router,
        gemini_model_candidate, plan_route_chain,
    },
    types::{
        extensions::{CallerRequestContext, WorkUnitSource},
        router::RouterId,
    },
};

fn routing_load_key(requirements: &RequestRequirements) -> RouteMemoryKey {
    RouteMemoryKey::for_route_class(
        &RouterId::Named("routing-load".into()),
        EndpointType::Chat,
        requirements,
        None,
        None,
        RouteStreamMode::NonStreaming,
    )
}

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
    let requirements = RequestRequirements::default();
    let key = routing_load_key(&requirements);
    memory
        .record(
            &key,
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
        &requirements,
        None,
        &caller,
        &CredentialHealthRegistry::new(),
        memory,
        100,
        &HashSet::new(),
        None,
        RouteStreamMode::NonStreaming,
    )
    .await;
    assert!(plan.route_memory_hit);
    assert_eq!(plan.chain[0].credential_id.as_str(), "gemini-free-9");
}

#[tokio::test]
async fn route_memory_is_gateway_level_not_work_unit_level() {
    let app_state = AppState::test_default().await;
    let memory = app_state.route_memory();
    let requirements = RequestRequirements::default();
    let key = routing_load_key(&requirements);
    memory
        .record(
            &key,
            RouteBinding {
                credential_id: ProviderCredentialId::new("gemini-free-9"),
                model: "gemini-3.1-flash-lite".to_string(),
            },
        )
        .await;
    let caller = CallerRequestContext {
        agent_name: "another-invoker".to_string(),
        work_unit_id: Some("different-unit".to_string()),
        work_unit_source: WorkUnitSource::Explicit,
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
        &requirements,
        None,
        &caller,
        &CredentialHealthRegistry::new(),
        memory,
        100,
        &HashSet::new(),
        None,
        RouteStreamMode::NonStreaming,
    )
    .await;
    assert!(plan.route_memory_hit);
    assert_eq!(plan.chain[0].credential_id.as_str(), "gemini-free-9");
}

#[tokio::test]
async fn route_class_memory_does_not_pin_equivalent_credentials() {
    let app_state = AppState::test_default().await;
    let router = empty_router(&app_state);
    let memory = app_state.route_memory();
    let requirements = RequestRequirements::default();
    let key = routing_load_key(&requirements);
    memory
        .record(
            &key,
            RouteBinding {
                credential_id: ProviderCredentialId::new("gemini-free-9"),
                model: "gemini-3.1-flash-lite".to_string(),
            },
        )
        .await;
    let caller = CallerRequestContext {
        agent_name: "spread-agent".to_string(),
        work_unit_id: Some("unit-1".to_string()),
        work_unit_source: WorkUnitSource::Explicit,
    };
    let spread_peer = gemini_model_candidate(
        &app_state,
        "gemini-free-1",
        "gemini-3.1-flash-lite",
    )
    .await;
    let preferred = gemini_model_candidate(
        &app_state,
        "gemini-free-9",
        "gemini-3.1-flash-lite",
    )
    .await;

    let plan = plan_route_chain(
        &router,
        vec![spread_peer, preferred],
        &requirements,
        None,
        &caller,
        &CredentialHealthRegistry::new(),
        memory,
        100,
        &HashSet::new(),
        None,
        RouteStreamMode::NonStreaming,
    )
    .await;

    assert!(plan.route_memory_hit);
    assert_eq!(plan.chain[0].credential_id.as_str(), "gemini-free-1");
    assert!(
        plan.chain.iter().any(
            |candidate| candidate.credential_id.as_str() == "gemini-free-9"
        ),
        "remembered binding must stay in the plan"
    );
}
