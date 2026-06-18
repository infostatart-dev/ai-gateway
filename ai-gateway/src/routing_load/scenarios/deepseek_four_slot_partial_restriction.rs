//! `DeepSeek` with four session slots: partial restriction and full exhaustion.
//!
//! `deepseek_credential_restricted_failover` already covers two slots (1/2
//! muted). Here we only add scale (3/4) and terminal 403 when every slot is
//! restricted.

use http::StatusCode;

use crate::{
    app_state::AppState,
    router::{
        budget_aware::{
            balance_ranked, clear_test_call_responses, deepseek_slots,
            empty_router, push_test_call_response_for_credential,
            request_parts, run_failover_candidates,
        },
        capability::RequestRequirements,
    },
    routing_load::{
        assert_identity::routed_identity,
        payload::default_fat_body,
        responses::{credential_restricted, ok_chat_completion},
    },
    types::response::Response,
};

async fn failover_over_four_slots(
    app_state: &AppState,
    expect: &str,
) -> Response {
    let router = empty_router(app_state);
    let ranked = balance_ranked(&router, deepseek_slots(app_state, 4).await);
    run_failover_candidates(
        router,
        request_parts(),
        default_fat_body(),
        ranked,
        RequestRequirements::default(),
        None,
    )
    .await
    .expect(expect)
}

pub async fn run_three_of_four_muted() {
    clear_test_call_responses();
    for slot in ["deepseek-web-default", "deepseek-web-2", "deepseek-web-3"] {
        push_test_call_response_for_credential(
            slot,
            Ok(credential_restricted(None)),
        );
    }
    push_test_call_response_for_credential(
        "deepseek-web-4",
        Ok(ok_chat_completion()),
    );

    let app_state = AppState::test_default().await;
    let response = failover_over_four_slots(
        &app_state,
        "three of four slots muted — fourth slot succeeds",
    )
    .await;

    assert_eq!(
        routed_identity(&response),
        "deepseek-web-4/deepseek-chat",
        "first healthy slot after three restricted siblings"
    );
}

pub async fn run_all_four_muted() {
    clear_test_call_responses();
    for slot in [
        "deepseek-web-default",
        "deepseek-web-2",
        "deepseek-web-3",
        "deepseek-web-4",
    ] {
        push_test_call_response_for_credential(
            slot,
            Ok(credential_restricted(None)),
        );
    }

    let app_state = AppState::test_default().await;
    let response = failover_over_four_slots(
        &app_state,
        "all four slots restricted — terminal 403",
    )
    .await;

    assert_eq!(
        response.status(),
        StatusCode::FORBIDDEN,
        "all four slots restricted — client sees credential restriction"
    );
}
