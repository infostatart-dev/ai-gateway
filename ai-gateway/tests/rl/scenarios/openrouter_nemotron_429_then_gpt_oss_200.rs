use ai_gateway::{
    app_state::AppState,
    tests::routing::{
        RequestRequirements, balance_ranked, clear_test_call_responses,
        empty_router, install_upstream_mock, openrouter_model_candidate,
        request_parts, run_failover_candidates,
    },
};
use gateway_tests::{
    UpstreamMockScript,
    upstream::{ok_chat_completion, openrouter_free_models_per_day_429},
};

use crate::rl::support::*;

pub async fn run() {
    clear_test_call_responses();
    install_upstream_mock(
        UpstreamMockScript::new()
            .binding(
                "openrouter-default",
                "nvidia/nemotron-3-nano-30b-a3b:free",
                vec![openrouter_free_models_per_day_429],
            )
            .binding(
                "openrouter-default",
                "openai/gpt-oss-120b:free",
                vec![ok_chat_completion],
            ),
    );

    let app_state = AppState::test_default().await;
    let router = empty_router(&app_state);
    let ranked = vec![
        openrouter_model_candidate(
            &app_state,
            "openrouter-default",
            "nvidia/nemotron-3-nano-30b-a3b:free",
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
    .expect("nemotron 429 retires model and gpt-oss succeeds");

    assert_eq!(
        routed_identity(&response),
        "openrouter-default/openai/gpt-oss-120b:free"
    );
}
