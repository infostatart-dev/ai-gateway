use ai_gateway::{
    app_state::AppState,
    tests::routing::{
        RequestRequirements, clear_test_call_responses, empty_router,
        gemini_model_candidate, install_upstream_mock,
    },
};
use gateway_tests::{UpstreamMockScript, upstream::ok_chat_completion};

use crate::rl::support::*;

const CRED: &str = "gemini-free-8";
const PREVIEW: &str = "gemini-3-flash-preview";
const FLASH_LITE: &str = "gemini-3.1-flash-lite";
/// Catalog RPD for `gemini-3-flash` free tier (preview slug normalizes here).
const PREVIEW_RPD: u32 = 20;

fn catalog_script() -> UpstreamMockScript {
    UpstreamMockScript::new()
        .binding(CRED, PREVIEW, vec![ok_chat_completion])
        .binding(CRED, FLASH_LITE, vec![ok_chat_completion])
        .default_response(ok_chat_completion)
}

pub async fn run() {
    clear_test_call_responses();
    install_upstream_mock(catalog_script());

    let app_state = AppState::test_default().await;
    saturate_model_pacing(&app_state, CRED, PREVIEW, PREVIEW_RPD).await;

    let router = empty_router(&app_state);
    let pool = vec![
        gemini_model_candidate(&app_state, CRED, PREVIEW).await,
        gemini_model_candidate(&app_state, CRED, FLASH_LITE).await,
    ];

    let planned = run_planned_failover(
        router,
        caller_parts("catalog-skip", Some("unit-rpd")),
        default_fat_body(),
        pool,
        RequestRequirements::default(),
        None,
    )
    .await
    .expect("flash-lite when preview RPD exhausted");

    let identity = routed_identity(&planned.response);
    assert!(
        !identity.contains(PREVIEW),
        "first hop must not target saturated preview, got {identity}"
    );
    assert_eq!(identity, format!("{CRED}/{FLASH_LITE}"));
}
