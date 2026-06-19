use std::collections::HashSet;

use ai_gateway::{
    app_state::AppState,
    tests::routing::{
        RequestRequirements, clear_test_call_responses, empty_router,
        gemini_model_candidate, install_upstream_mock,
        openrouter_model_candidate,
    },
};
use futures::future::join_all;
use gateway_tests::{UpstreamMockScript, upstream::ok_chat_completion};

use crate::rl::{helpers::trip_circuit, support::*};

const AGENT: &str = "invoker";
const MODEL: &str = "gemini-3.1-flash-lite";

fn dead_pool_script() -> UpstreamMockScript {
    let mut script = UpstreamMockScript::new();
    for index in 2..=8 {
        script = script.binding(
            format!("gemini-free-{index}"),
            MODEL,
            vec![gateway_tests::upstream::rate_limited_rpm],
        );
    }
    script
        .binding("gemini-free-9", MODEL, vec![ok_chat_completion])
        .binding("gemini-free-10", MODEL, vec![ok_chat_completion])
        .default_response(ok_chat_completion)
}

pub async fn run() {
    clear_test_call_responses();
    install_upstream_mock(dead_pool_script());

    let app_state = AppState::test_default().await;
    for index in 2..=8 {
        trip_circuit(&app_state, &format!("gemini-free-{index}"));
    }

    let dead_before = credential_attempts(&app_state, "gemini-free-2");
    let router = empty_router(&app_state);
    let mut pool = Vec::new();
    for index in 2..=10 {
        pool.push(
            gemini_model_candidate(
                &app_state,
                &format!("gemini-free-{index}"),
                MODEL,
            )
            .await,
        );
    }
    pool.push(
        openrouter_model_candidate(
            &app_state,
            "openrouter-default",
            "openai/gpt-oss-120b:free",
        )
        .await,
    );

    let body = default_fat_body();
    let futures = (1..=3).map(|unit| {
        let router = router.clone();
        let pool = pool.clone();
        let body = body.clone();
        async move {
            run_planned_failover(
                router,
                caller_parts(AGENT, Some(&format!("unit-{unit}"))),
                body,
                pool,
                RequestRequirements::default(),
                None,
            )
            .await
            .expect("healthy route")
        }
    });
    let results = join_all(futures).await;

    let mut healthy = HashSet::new();
    for result in &results {
        let identity = routed_identity(&result.response);
        let cred = identity.split('/').next().unwrap_or_default();
        if cred == "gemini-free-9" || cred == "gemini-free-10" {
            healthy.insert(cred.to_string());
        }
    }
    assert!(
        healthy.len() >= 2,
        "expected spread across healthy credentials, got {healthy:?}"
    );
    assert_eq!(
        credential_attempts(&app_state, "gemini-free-2"),
        dead_before,
        "circuit-open credential must not receive follow-up attempts"
    );
}
