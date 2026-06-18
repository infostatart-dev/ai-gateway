use crate::{
    app_state::AppState,
    router::{
        budget_aware::{
            balance_ranked, clear_test_call_responses, empty_router,
            gemini_model_candidate, push_test_call_response_for_credential,
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
    push_test_call_response_for_credential(
        "gemini-free-8",
        Ok(rate_limited_rpm()),
    );
    push_test_call_response_for_credential(
        "gemini-free-8",
        Ok(ok_chat_completion()),
    );

    let app_state = AppState::test_default().await;
    let router = empty_router(&app_state);
    let ranked = vec![
        gemini_model_candidate(
            &app_state,
            "gemini-free-8",
            "gemini-3-flash-preview",
        )
        .await,
        gemini_model_candidate(
            &app_state,
            "gemini-free-8",
            "gemini-3.1-flash-lite",
        )
        .await,
    ];
    let response = run_failover_candidates(
        router.clone(),
        request_parts(),
        default_fat_body(),
        balance_ranked(&router, ranked),
        RequestRequirements::default(),
        None,
    )
    .await
    .expect("intra-slot ladder success");

    assert_eq!(
        routed_identity(&response),
        "gemini-free-8/gemini-3.1-flash-lite"
    );
}
