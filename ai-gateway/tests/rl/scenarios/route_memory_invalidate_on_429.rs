use ai_gateway::{
    app_state::AppState,
    tests::routing::{
        RequestRequirements, clear_test_call_responses, empty_router,
        gemini_model_candidate, install_upstream_mock,
    },
};
use gateway_tests::{UpstreamMockScript, upstream::ok_chat_completion};

use crate::rl::support::*;

const AGENT: &str = "memory-agent";
const UNIT: &str = "unit-47";
const MODEL: &str = "gemini-3.1-flash-lite";
const CRED_NINE: &str = "gemini-free-9";
const CRED_TEN: &str = "gemini-free-10";

pub async fn run() {
    clear_test_call_responses();
    install_upstream_mock(
        UpstreamMockScript::new()
            .binding(
                CRED_NINE,
                MODEL,
                vec![
                    ok_chat_completion,
                    gateway_tests::upstream::rate_limited_rpm,
                ],
            )
            .credential(CRED_TEN, vec![ok_chat_completion])
            .default_response(ok_chat_completion),
    );

    let app_state = AppState::test_default().await;
    let router = empty_router(&app_state);
    let only_nine =
        vec![gemini_model_candidate(&app_state, CRED_NINE, MODEL).await];
    let dual = vec![
        gemini_model_candidate(&app_state, CRED_NINE, MODEL).await,
        gemini_model_candidate(&app_state, CRED_TEN, MODEL).await,
    ];
    let body = default_fat_body();

    let seed = run_planned_failover(
        router.clone(),
        caller_parts(AGENT, Some(UNIT)),
        body.clone(),
        only_nine,
        RequestRequirements::default(),
        None,
    )
    .await
    .expect("seed binding");
    assert_eq!(
        routed_identity(&seed.response),
        format!("{CRED_NINE}/{MODEL}")
    );

    let after_429 = run_planned_failover(
        router,
        caller_parts(AGENT, Some(UNIT)),
        body,
        dual,
        RequestRequirements::default(),
        None,
    )
    .await
    .expect("failover after binding 429");

    assert!(after_429.route_memory_hit);
    assert_eq!(
        routed_identity(&after_429.response),
        format!("{CRED_TEN}/{MODEL}")
    );
    let stored = app_state
        .route_memory()
        .get(AGENT, UNIT)
        .await
        .expect("success records new binding");
    assert_eq!(stored.credential_id.as_str(), CRED_TEN);
    assert_eq!(stored.model, MODEL);
}
