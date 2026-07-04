use ai_gateway::{
    app_state::AppState,
    tests::routing::{
        RequestRequirements, clear_test_call_responses, empty_router,
        gemini_model_candidate, hold_candidate_route_lease,
        install_upstream_mock, named_model_candidate,
    },
};
use gateway_tests::{UpstreamMockScript, upstream::ok_chat_completion};

use crate::rl::{helpers::trip_circuit, support::*};

const GEMINI_CREDENTIAL: &str = "gemini-free";
const GEMINI_MODEL: &str = "gemini-3.1-flash-lite";
const VLLM_CREDENTIAL: &str = "vllm-anonymous";
const VLLM_MODEL: &str = "am-thinking-awq";

pub async fn run() {
    clear_test_call_responses();
    install_upstream_mock(
        UpstreamMockScript::new().default_response(ok_chat_completion),
    );

    let app_state = AppState::test_default().await;
    trip_circuit(&app_state, GEMINI_CREDENTIAL);

    let router = empty_router(&app_state);
    let vllm = named_model_candidate(
        &app_state,
        "vllm",
        VLLM_CREDENTIAL,
        VLLM_MODEL,
        128_000,
    )
    .await;
    let gemini =
        gemini_model_candidate(&app_state, GEMINI_CREDENTIAL, GEMINI_MODEL)
            .await;
    let _held_vllm =
        hold_candidate_route_lease(&router, &vllm).expect("held vllm lease");

    let result = run_planned_failover(
        router,
        caller_parts("route-lease", Some("unit-exhausted")),
        default_fat_body(),
        vec![vllm, gemini],
        RequestRequirements::default(),
        None,
    )
    .await
    .expect("exhausted route should return a typed retry response");

    assert_eq!(
        result.response.status(),
        http::StatusCode::TOO_MANY_REQUESTS
    );
    assert_eq!(
        result
            .response
            .headers()
            .get("x-ratelimit-remaining")
            .unwrap(),
        "0"
    );
    assert!(result.response.headers().get("retry-after").is_some());
}
