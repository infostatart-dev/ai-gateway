use crate::{
    app_state::AppState,
    router::{
        budget_aware::{
            balance_ranked, clear_test_call_responses,
            deepseek_model_candidate, empty_router,
            push_test_call_response_for_credential, request_parts,
            run_failover_candidates,
        },
        capability::RequestRequirements,
    },
    routing_load::{
        assert_identity::routed_identity,
        payload::default_fat_body,
        responses::{credential_restricted, ok_chat_completion},
    },
};

pub async fn run() {
    clear_test_call_responses();
    push_test_call_response_for_credential(
        "deepseek-web-default",
        Ok(credential_restricted(None)),
    );
    push_test_call_response_for_credential(
        "deepseek-web-2",
        Ok(ok_chat_completion()),
    );

    let app_state = AppState::test_default().await;
    let router = empty_router(&app_state);
    let ranked = vec![
        deepseek_model_candidate(
            &app_state,
            "deepseek-web-default",
            "deepseek-chat",
        )
        .await,
        deepseek_model_candidate(&app_state, "deepseek-web-2", "deepseek-chat")
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
    .expect("second deepseek slot succeeds after restriction");

    assert_eq!(routed_identity(&response), "deepseek-web-2/deepseek-chat");
}
