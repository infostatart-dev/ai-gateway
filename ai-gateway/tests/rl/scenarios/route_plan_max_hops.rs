use ai_gateway::{
    app_state::AppState,
    tests::routing::{
        RequestRequirements, clear_test_call_responses, empty_router,
        gemini_model_candidate, install_upstream_mock,
    },
};
use gateway_tests::{UpstreamMockScript, upstream::ok_chat_completion};

use crate::rl::support::*;

const CRED: &str = "gemini-free-8";
const MAX_HOPS: u32 = 7;

fn ladder_script() -> UpstreamMockScript {
    UpstreamMockScript::new()
        .binding(
            CRED,
            "gemini-3-flash-preview",
            vec![gateway_tests::upstream::rate_limited_rpm],
        )
        .binding(
            CRED,
            "gemini-3.5-flash",
            vec![gateway_tests::upstream::rate_limited_rpm],
        )
        .binding(
            CRED,
            "gemini-3.1-flash-lite",
            vec![gateway_tests::upstream::rate_limited_rpm],
        )
        .binding(
            CRED,
            "gemini-2.5-flash",
            vec![gateway_tests::upstream::rate_limited_rpm],
        )
        .binding(CRED, "gemini-2.5-flash-lite", vec![ok_chat_completion])
        .default_response(ok_chat_completion)
}

pub async fn run() {
    clear_test_call_responses();
    install_upstream_mock(ladder_script());

    let app_state = AppState::test_default().await;
    let router = empty_router(&app_state);
    let pool = vec![
        gemini_model_candidate(&app_state, CRED, "gemini-3-flash-preview")
            .await,
        gemini_model_candidate(&app_state, CRED, "gemini-3.5-flash").await,
        gemini_model_candidate(&app_state, CRED, "gemini-3.1-flash-lite").await,
        gemini_model_candidate(&app_state, CRED, "gemini-2.5-flash").await,
        gemini_model_candidate(&app_state, CRED, "gemini-2.5-flash-lite").await,
    ];

    let planned = run_planned_failover(
        router,
        caller_parts("hop-cap", Some("unit-hop")),
        default_fat_body(),
        pool,
        RequestRequirements::default(),
        None,
    )
    .await
    .expect("terminal success within hop budget");

    assert!(planned.planned_hops <= MAX_HOPS);
    assert_eq!(
        routed_identity(&planned.response),
        format!("{CRED}/gemini-2.5-flash-lite")
    );
}
