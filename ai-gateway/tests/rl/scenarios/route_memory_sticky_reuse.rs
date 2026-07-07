use ai_gateway::{
    app_state::AppState,
    tests::routing::{
        RequestRequirements, clear_test_call_responses, empty_router,
        gemini_model_candidate, install_upstream_mock,
    },
};
use gateway_tests::{UpstreamMockScript, upstream::ok_chat_completion};

use crate::rl::support::*;

const AGENT: &str = "memory-agent";
const OTHER_AGENT: &str = "other-memory-agent";
const UNIT: &str = "unit-47";
const OTHER_UNIT: &str = "unit-99";
const MODEL: &str = "gemini-3.1-flash-lite";
const CRED: &str = "gemini-free-9";

pub async fn run() {
    clear_test_call_responses();
    install_upstream_mock(
        UpstreamMockScript::new()
            .binding(CRED, MODEL, vec![ok_chat_completion])
            .default_response(ok_chat_completion),
    );

    let app_state = AppState::test_default().await;
    let router = empty_router(&app_state);
    let pool = vec![gemini_model_candidate(&app_state, CRED, MODEL).await];
    let body = default_fat_body();

    let first = run_planned_failover(
        router.clone(),
        caller_parts(AGENT, Some(UNIT)),
        body.clone(),
        pool.clone(),
        RequestRequirements::default(),
        None,
    )
    .await
    .expect("seed binding");
    assert_eq!(routed_identity(&first.response), format!("{CRED}/{MODEL}"));
    assert!(!first.route_memory_hit);

    let second = run_planned_failover(
        router,
        caller_parts(OTHER_AGENT, Some(OTHER_UNIT)),
        body,
        pool,
        RequestRequirements::default(),
        None,
    )
    .await
    .expect("sticky reuse");

    assert!(second.route_memory_hit);
    assert_eq!(routed_identity(&second.response), format!("{CRED}/{MODEL}"));
}
