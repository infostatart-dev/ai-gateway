use ai_gateway::{
    app_state::AppState,
    tests::routing::{
        RequestRequirements, balance_ranked, clear_test_call_responses,
        deepseek_model_candidate, empty_router, gemini_model_candidate,
        install_upstream_mock, request_parts, run_failover_candidates,
    },
};
use gateway_tests::{
    UpstreamMockScript,
    upstream::{
        credential_restricted_default, daily_quota_exhausted,
        ok_chat_completion,
    },
};

use crate::rl::support::*;

pub async fn run() {
    clear_test_call_responses();
    install_upstream_mock(
        UpstreamMockScript::new()
            .credential(
                "deepseek-web-default",
                vec![credential_restricted_default],
            )
            .credential("deepseek-web-2", vec![credential_restricted_default])
            .binding(
                "gemini-free-8",
                "gemini-3-flash-preview",
                vec![daily_quota_exhausted],
            )
            .binding(
                "gemini-free-8",
                "gemini-3.5-flash",
                vec![daily_quota_exhausted],
            )
            .binding(
                "gemini-free-8",
                "gemini-3.1-flash-lite",
                vec![daily_quota_exhausted],
            )
            .binding(
                "gemini-free-8",
                "gemini-2.5-flash",
                vec![daily_quota_exhausted],
            )
            .binding(
                "gemini-free-8",
                "gemini-2.5-flash-lite",
                vec![ok_chat_completion],
            ),
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
