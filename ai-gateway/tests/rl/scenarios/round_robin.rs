use ai_gateway::{
    app_state::AppState,
    tests::routing::{
        RequestRequirements, balance_ranked, clear_test_call_responses,
        empty_router, gemini_slots, install_upstream_mock, request_parts,
        run_failover_candidates,
    },
};
use futures::future::join_all;
use gateway_tests::{UpstreamMockScript, upstream::ok_chat_completion};

use crate::rl::support::*;

const FREE_SLOTS: [&str; 4] = [
    "gemini-free",
    "gemini-free-2",
    "gemini-free-3",
    "gemini-free-4",
];

pub async fn run() {
    clear_test_call_responses();
    install_upstream_mock(
        UpstreamMockScript::new().default_response(ok_chat_completion),
    );

    let app_state = AppState::test_default().await;
    let router = empty_router(&app_state);
    let ranked = gemini_slots(&app_state, 4).await;
    let parts = request_parts();
    let body = default_fat_body();

    let futures = (0..32).map(|_| {
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
            .expect("success");
            routed_identity(&response)
        }
    });
    let identities: Vec<_> = join_all(futures).await;
    let counts = terminal_provider_counts(&identities);
    assert_fairness_band(&counts, &FREE_SLOTS, 32, 25);
    assert_zero_terminal_credentials(&counts, &["chatgpt-web-default"]);
}
