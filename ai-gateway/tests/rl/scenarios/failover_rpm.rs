use ai_gateway::{
    app_state::AppState,
    tests::routing::{
        RequestRequirements, balance_ranked, clear_test_call_responses,
        empty_router, gemini_candidate, install_upstream_mock, request_parts,
        run_failover_candidates,
    },
};
use futures::future::join_all;
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
            .credential("gemini-free-2", vec![ok_chat_completion])
            .default_response(ok_chat_completion),
    );

    let app_state = AppState::test_default().await;
    let router = empty_router(&app_state);
    let ranked = vec![
        gemini_candidate(&app_state, "gemini-free", 0, "free-1-key").await,
        gemini_candidate(&app_state, "gemini-free-2", 0, "free-2-key").await,
        gemini_candidate(&app_state, "gemini-default", 10, "paid-key").await,
    ];
    let parts = request_parts();
    let body = default_fat_body();

    let futures = (0..8).map(|_| {
        let router = router.clone();
        let ranked = ranked.clone();
        let parts = parts.clone();
        let body = body.clone();
        async move {
            let candidates = balance_ranked(&router, ranked);
            let response = run_failover_candidates(
                router,
                parts,
                body,
                candidates,
                RequestRequirements::default(),
                None,
            )
            .await
            .expect("failover");
            routed_identity(&response)
        }
    });
    let identities: Vec<_> = join_all(futures).await;
    let counts = terminal_provider_counts(&identities);
    assert!(
        counts.get("gemini-free-2").copied().unwrap_or(0) >= 8,
        "expected sibling success"
    );
    assert_zero_terminal_credentials(&counts, &["chatgpt-web-default"]);
}
