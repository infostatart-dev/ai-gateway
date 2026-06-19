use ai_gateway::{
    app_state::AppState,
    tests::routing::{
        RequestRequirements, balance_ranked, chatgpt_candidate,
        clear_test_call_responses, empty_router, gemini_candidate,
        install_upstream_mock, request_parts, run_failover_candidates,
    },
};
use gateway_tests::{
    UpstreamMockScript,
    upstream::{ok_chat_completion, rate_limited_rpm},
};

use crate::rl::support::*;

pub async fn run() {
    clear_test_call_responses();
    install_upstream_mock(
        UpstreamMockScript::new()
            .credential("gemini-free", vec![rate_limited_rpm])
            .credential("gemini-default", vec![rate_limited_rpm])
            .credential("chatgpt-web-default", vec![ok_chat_completion]),
    );

    let app_state = AppState::test_default().await;
    let router = empty_router(&app_state);
    let ranked = vec![
        gemini_candidate(&app_state, "gemini-free", 0, "free-1-key").await,
        gemini_candidate(&app_state, "gemini-default", 10, "paid-key").await,
        chatgpt_candidate(&app_state).await,
    ];
    let candidates = balance_ranked(&router, ranked);
    let response = run_failover_candidates(
        router,
        request_parts(),
        default_fat_body(),
        candidates,
        RequestRequirements::default(),
        None,
    )
    .await
    .expect("chatgpt last resort");

    assert!(
        routed_identity(&response).starts_with("chatgpt-web-default/"),
        "expected chatgpt-web terminal"
    );
}
