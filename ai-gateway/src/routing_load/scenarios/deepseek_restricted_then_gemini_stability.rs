use crate::{
    app_state::AppState,
    router::{
        budget_aware::{
            balance_ranked, clear_test_call_responses,
            deepseek_model_candidate, empty_router, gemini_model_candidate,
            push_test_call_response_for_credential, request_parts,
            run_failover_candidates,
        },
        capability::RequestRequirements,
    },
    routing_load::{
        assert_identity::routed_identity,
        payload::default_fat_body,
        responses::{
            credential_restricted, daily_quota_exhausted, ok_chat_completion,
        },
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
        Ok(credential_restricted(None)),
    );
    for _ in 0..4 {
        push_test_call_response_for_credential(
            "gemini-free-8",
            Ok(daily_quota_exhausted()),
        );
    }
    push_test_call_response_for_credential(
        "gemini-free-8",
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
        gemini_model_candidate(
            &app_state,
            "gemini-free-8",
            "gemini-3-flash-preview",
        )
        .await,
        gemini_model_candidate(&app_state, "gemini-free-8", "gemini-3.5-flash")
            .await,
        gemini_model_candidate(
            &app_state,
            "gemini-free-8",
            "gemini-3.1-flash-lite",
        )
        .await,
        gemini_model_candidate(&app_state, "gemini-free-8", "gemini-2.5-flash")
            .await,
        gemini_model_candidate(
            &app_state,
            "gemini-free-8",
            "gemini-2.5-flash-lite",
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
    .expect("gemini stability succeeds after deepseek restriction");

    assert_eq!(
        routed_identity(&response),
        "gemini-free-8/gemini-2.5-flash-lite"
    );
}
