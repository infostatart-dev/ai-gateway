use ai_gateway::{
    app_state::AppState,
    tests::routing::{
        RequestRequirements, clear_test_call_responses, empty_router,
        groq_candidate, hold_candidate_route_lease, install_upstream_mock,
        named_model_candidate,
    },
};
use gateway_tests::{UpstreamMockScript, upstream::ok_chat_completion};

use crate::rl::support::*;

const VLLM_CREDENTIAL: &str = "vllm-anonymous";
const VLLM_MODEL: &str = "am-thinking-awq";

pub async fn run() {
    clear_test_call_responses();
    install_upstream_mock(
        UpstreamMockScript::new().default_response(ok_chat_completion),
    );

    let app_state = AppState::test_default().await;
    let router = empty_router(&app_state);
    let vllm = named_model_candidate(
        &app_state,
        "vllm",
        VLLM_CREDENTIAL,
        VLLM_MODEL,
        128_000,
    )
    .await;
    let groq = groq_candidate(&app_state).await;
    let _held_vllm =
        hold_candidate_route_lease(&router, &vllm).expect("held vllm lease");

    let result = run_planned_failover(
        router,
        caller_parts("route-lease", Some("unit-vllm-busy")),
        default_fat_body(),
        vec![vllm, groq],
        RequestRequirements::default(),
        None,
    )
    .await
    .expect("route around busy vllm");

    let identity = routed_identity(&result.response);
    assert!(
        !identity.starts_with(&format!("{VLLM_CREDENTIAL}/")),
        "busy VLLM slot must not be selected while a route lease is held, got \
         {identity}"
    );
    assert!(
        identity.starts_with("groq-default/"),
        "expected Groq fallback while VLLM lease is active, got {identity}"
    );
}
