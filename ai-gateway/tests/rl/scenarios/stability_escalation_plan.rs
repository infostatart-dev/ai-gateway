use ai_gateway::{
    app_state::AppState,
    tests::routing::{
        RequestRequirements, clear_test_call_responses, empty_router,
        gemini_model_candidate, install_upstream_mock,
        openrouter_model_candidate,
    },
};
use gateway_tests::{UpstreamMockScript, upstream::ok_chat_completion};

use crate::rl::support::*;

const CRED: &str = "gemini-free-8";

fn stability_script() -> UpstreamMockScript {
    UpstreamMockScript::new()
        .binding(
            CRED,
            "gemini-3-flash-preview",
            vec![gateway_tests::upstream::rate_limited_rpm],
        )
        .binding(
            CRED,
            "gemini-3.5-flash",
            vec![gateway_tests::upstream::rate_limited_rpm],
        )
        .binding(CRED, "gemini-3.1-flash-lite", vec![ok_chat_completion])
        .binding(
            "openrouter-default",
            "nvidia/nemotron-3-nano-30b-a3b:free",
            vec![ok_chat_completion],
        )
        .default_response(ok_chat_completion)
}

pub async fn run() {
    clear_test_call_responses();
    install_upstream_mock(stability_script());

    let app_state = AppState::test_default().await;
    let before_or = credential_attempts(&app_state, "openrouter-default");
    let router = empty_router(&app_state);
    let pool = vec![
        gemini_model_candidate(&app_state, CRED, "gemini-3-flash-preview")
            .await,
        gemini_model_candidate(&app_state, CRED, "gemini-3.5-flash").await,
        gemini_model_candidate(&app_state, CRED, "gemini-3.1-flash-lite").await,
        openrouter_model_candidate(
            &app_state,
            "openrouter-default",
            "nvidia/nemotron-3-nano-30b-a3b:free",
        )
        .await,
    ];

    let planned = run_planned_failover(
        router,
        caller_parts("stability-plan", Some("unit-stab")),
        default_fat_body(),
        pool,
        RequestRequirements::default(),
        None,
    )
    .await
    .expect("flash-lite before cross-provider");

    assert_eq!(
        routed_identity(&planned.response),
        format!("{CRED}/gemini-3.1-flash-lite")
    );
    assert_eq!(
        credential_attempts(&app_state, "openrouter-default"),
        before_or,
        "openrouter must not be attempted when gemini stability band succeeds"
    );
}
