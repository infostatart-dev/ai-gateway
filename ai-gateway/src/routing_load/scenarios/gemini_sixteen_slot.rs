use futures::future::join_all;

use crate::{
    app_state::AppState,
    router::{
        budget_aware::{
            balance_ranked, clear_test_call_responses, empty_router,
            gemini_slots, push_test_call_response, request_parts,
            run_failover_candidates,
        },
        capability::RequestRequirements,
    },
    routing_load::{
        assert_identity::{
            assert_fairness_band, assert_zero_terminal_credentials,
            routed_identity, terminal_provider_counts,
        },
        payload::default_fat_body,
        responses::ok_chat_completion,
    },
};

const FREE_SLOTS: [&str; 16] = [
    "gemini-free",
    "gemini-free-2",
    "gemini-free-3",
    "gemini-free-4",
    "gemini-free-5",
    "gemini-free-6",
    "gemini-free-7",
    "gemini-free-8",
    "gemini-free-9",
    "gemini-free-10",
    "gemini-free-11",
    "gemini-free-12",
    "gemini-free-13",
    "gemini-free-14",
    "gemini-free-15",
    "gemini-free-16",
];

pub async fn run() {
    clear_test_call_responses();
    for _ in 0..64 {
        push_test_call_response(Ok(ok_chat_completion()));
    }

    let app_state = AppState::test_default().await;
    let router = empty_router(&app_state);
    let ranked = gemini_slots(&app_state, 16).await;
    let parts = request_parts();
    let body = default_fat_body();

    let futures = (0..64).map(|_| {
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
    assert_fairness_band(&counts, &FREE_SLOTS, 64, 25);
    assert_zero_terminal_credentials(&counts, &["chatgpt-web-default"]);
}
