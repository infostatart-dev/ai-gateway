use ai_gateway::{
    app_state::AppState,
    tests::routing::{
        RequestRequirements, clear_test_call_responses, empty_router,
        gemini_model_candidate, install_upstream_mock,
    },
};
use gateway_tests::{UpstreamMockScript, upstream::rate_limited_rpm};

use crate::rl::support::*;

const MODEL: &str = "gemini-2.5-flash-lite";
const BLOCKED: &str = "gemini-free-4";
const SIBLING: &str = "gemini-free-5";

pub async fn run() {
    clear_test_call_responses();
    install_upstream_mock(
        UpstreamMockScript::new()
            .binding(BLOCKED, MODEL, vec![rate_limited_rpm])
            .binding(SIBLING, MODEL, vec![ok_chat_completion])
            .default_response(ok_chat_completion),
    );

    let app_state = AppState::test_default().await;
    let router = empty_router(&app_state);
    let pool = vec![
        gemini_model_candidate(&app_state, BLOCKED, MODEL).await,
        gemini_model_candidate(&app_state, SIBLING, MODEL).await,
    ];
    let body = default_fat_body();

    let first = run_planned_failover(
        router.clone(),
        caller_parts("repeat-429", Some("unit-1")),
        body.clone(),
        pool.clone(),
        RequestRequirements::default(),
        None,
    )
    .await
    .expect("first hop may failover after 429");
    assert!(
        routed_identity(&first.response).starts_with(SIBLING),
        "expected sibling after reconcile, got {:?}",
        routed_identity(&first.response)
    );

    let second = run_planned_failover(
        router,
        caller_parts("repeat-429", Some("unit-2")),
        body,
        pool,
        RequestRequirements::default(),
        None,
    )
    .await
    .expect("second request");
    assert!(
        routed_identity(&second.response).starts_with(SIBLING),
        "blocked scope must not be re-hit, got {:?}",
        routed_identity(&second.response)
    );

    let snapshot = app_state.provider_stats_snapshot(None, None);
    assert_eq!(
        snapshot.routing.repeat_429_violations, 0,
        "admission must prevent repeat 429 on infeasible scope"
    );
    assert_eq!(
        credential_attempts(&app_state, BLOCKED),
        1,
        "blocked credential should see exactly one upstream attempt"
    );
    assert_eq!(
        app_state
            .provider_stats_snapshot(None, None)
            .routing
            .repeat_429_violations,
        0
    );
}
