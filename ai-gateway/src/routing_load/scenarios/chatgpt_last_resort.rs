use crate::{
    app_state::AppState,
    router::{
        budget_aware::{
            balance_ranked, chatgpt_candidate, clear_test_call_responses,
            empty_router, gemini_candidate, push_test_call_response,
            request_parts, run_failover_candidates,
        },
        capability::RequestRequirements,
    },
    routing_load::{
        assert_identity::routed_identity,
        payload::default_fat_body,
        responses::{ok_chat_completion, rate_limited_rpm},
    },
};

pub async fn run() {
    clear_test_call_responses();
    push_test_call_response(Ok(rate_limited_rpm()));
    push_test_call_response(Ok(rate_limited_rpm()));
    push_test_call_response(Ok(ok_chat_completion()));

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
