use ai_gateway::{
    app_state::AppState,
    tests::routing::{
        RequestRequirements, balance_ranked, clear_test_call_responses,
        empty_router, gemini_candidate, install_upstream_mock, request_parts,
        run_failover_candidates,
    },
};
use gateway_tests::{
    UpstreamMockScript,
    upstream::{ok_chat_completion, project_billing_exhausted},
};

use crate::rl::support::*;

pub async fn run() {
    clear_test_call_responses();
    install_upstream_mock(
        UpstreamMockScript::new()
            .credential("gemini-free", vec![project_billing_exhausted])
            .credential("gemini-free-2", vec![project_billing_exhausted])
            .credential("gemini-free-3", vec![project_billing_exhausted])
            .credential("gemini-default", vec![ok_chat_completion]),
    );

    let app_state = AppState::test_default().await;
    let router = empty_router(&app_state);
    let ranked = vec![
        gemini_candidate(&app_state, "gemini-free", 0, "free-1-key").await,
        gemini_candidate(&app_state, "gemini-free-2", 0, "free-2-key").await,
        gemini_candidate(&app_state, "gemini-free-3", 0, "free-3-key").await,
        gemini_candidate(&app_state, "gemini-default", 10, "paid-key").await,
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
    .expect("paid gemini succeeds");

    assert_eq!(
        routed_identity(&response),
        "gemini-default/gemini-2.5-flash"
    );
}
