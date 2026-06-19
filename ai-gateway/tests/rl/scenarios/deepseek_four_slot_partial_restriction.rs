use ai_gateway::{
    app_state::AppState,
    tests::routing::{
        RequestRequirements, balance_ranked, clear_test_call_responses,
        deepseek_slots, empty_router, install_upstream_mock, request_parts,
        run_failover_candidates,
    },
    types::response::Response,
};
use gateway_tests::{
    UpstreamMockScript,
    upstream::{credential_restricted_default, ok_chat_completion},
};
use http::StatusCode;

use crate::rl::support::*;

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

pub async fn run() {
    run_three_of_four_muted().await;
    run_all_four_muted().await;
}

pub async fn run_three_of_four_muted() {
    clear_test_call_responses();
    install_upstream_mock(
        UpstreamMockScript::new()
            .credential(
                "deepseek-web-default",
                vec![credential_restricted_default],
            )
            .credential("deepseek-web-2", vec![credential_restricted_default])
            .credential("deepseek-web-3", vec![credential_restricted_default])
            .credential("deepseek-web-4", vec![ok_chat_completion]),
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
    install_upstream_mock(
        UpstreamMockScript::new()
            .credential(
                "deepseek-web-default",
                vec![credential_restricted_default],
            )
            .credential("deepseek-web-2", vec![credential_restricted_default])
            .credential("deepseek-web-3", vec![credential_restricted_default])
            .credential("deepseek-web-4", vec![credential_restricted_default]),
    );

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
