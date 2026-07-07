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
const BLOCKED: &str = "gemini-free-6";
const SIBLING: &str = "gemini-free-7";

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
    let blocked = gemini_model_candidate(&app_state, BLOCKED, MODEL).await;
    let sibling = gemini_model_candidate(&app_state, SIBLING, MODEL).await;
    let pool = vec![blocked.clone(), sibling];

    let result = run_planned_failover(
        router,
        caller_parts("hop-readmit", Some("unit-1")),
        default_fat_body(),
        pool,
        RequestRequirements::default(),
        None,
    )
    .await
    .expect("hop readmit failover");

    assert_eq!(
        routed_identity(&result.response),
        format!("{SIBLING}/{MODEL}")
    );
    assert_eq!(
        app_state.credential_health().model_attempts(
            &blocked.capability.provider,
            &blocked.credential_id,
            MODEL,
        ),
        1,
        "re-admit must not repeat upstream on infeasible scope"
    );
    assert_eq!(
        app_state
            .provider_stats_snapshot(None, None)
            .routing
            .repeat_429_violations,
        0
    );
}
