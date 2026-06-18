use crate::{
    app_state::AppState,
    router::{
        budget_aware::{
            balance_ranked, clear_test_call_responses, empty_router,
            openrouter_model_candidate, push_test_call_response_for_credential,
            request_parts, run_failover_candidates,
        },
        capability::RequestRequirements,
    },
    routing_load::{
        assert_identity::routed_identity,
        payload::default_fat_body,
        responses::{ok_chat_completion, openrouter_never_purchased_402},
    },
};

pub async fn run() {
    clear_test_call_responses();
    push_test_call_response_for_credential(
        "openrouter-default",
        Ok(openrouter_never_purchased_402()),
    );
    push_test_call_response_for_credential(
        "openrouter-default",
        Ok(ok_chat_completion()),
    );

    let app_state = AppState::test_default().await;
    let router = empty_router(&app_state);
    let ranked = vec![
        openrouter_model_candidate(
            &app_state,
            "openrouter-default",
            "openai/gpt-4o-mini",
        )
        .await,
        openrouter_model_candidate(
            &app_state,
            "openrouter-default",
            "openai/gpt-oss-120b:free",
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
    .expect("paid 402 retires model and free sibling succeeds");

    assert_eq!(
        routed_identity(&response),
        "openrouter-default/openai/gpt-oss-120b:free"
    );
}
