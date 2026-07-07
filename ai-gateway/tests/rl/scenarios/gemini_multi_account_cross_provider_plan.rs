//! Reproduces the dev monoprovider failure: eight Gemini accounts plus
//! `OpenRouter` must all stay present in the plan so cross-provider failover
//! is guaranteed after Gemini ladder exhaustion.

use ai_gateway::{
    app_state::AppState,
    tests::routing::{
        BudgetCandidate, RequestRequirements, clear_test_call_responses,
        empty_router, gemini_model_candidate, install_upstream_mock,
        openrouter_model_candidate,
    },
};
use gateway_tests::{
    UpstreamMockScript,
    upstream::{high_demand_503, ok_chat_completion},
};

use crate::rl::support::*;

const GEMINI_SLOTS: [&str; 8] = [
    "gemini-free",
    "gemini-free-2",
    "gemini-free-3",
    "gemini-free-4",
    "gemini-free-5",
    "gemini-free-6",
    "gemini-free-7",
    "gemini-free-8",
];

const LADDER_MODELS: [&str; 4] = [
    "gemini-3-flash-preview",
    "gemini-3.5-flash",
    "gemini-3.1-flash-lite",
    "gemini-2.5-flash-lite",
];

const OPENROUTER_MODEL: &str = "openai/gpt-oss-120b:free";

fn all_gemini_overloaded_script() -> UpstreamMockScript {
    let mut script = UpstreamMockScript::new();
    for slot in GEMINI_SLOTS {
        for model in LADDER_MODELS {
            script = script.binding(slot, model, vec![high_demand_503]);
        }
    }
    script
        .binding(
            "openrouter-default",
            OPENROUTER_MODEL,
            vec![ok_chat_completion],
        )
        .default_response(high_demand_503)
}

async fn multi_account_gemini_pool(
    app_state: &AppState,
) -> Vec<BudgetCandidate> {
    let mut pool = Vec::new();
    for slot in GEMINI_SLOTS {
        for model in LADDER_MODELS {
            pool.push(gemini_model_candidate(app_state, slot, model).await);
        }
    }
    pool.push(
        openrouter_model_candidate(
            app_state,
            "openrouter-default",
            OPENROUTER_MODEL,
        )
        .await,
    );
    pool
}

pub async fn run() {
    clear_test_call_responses();
    install_upstream_mock(all_gemini_overloaded_script());

    let app_state = AppState::test_default().await;
    let router = empty_router(&app_state);
    let pool = multi_account_gemini_pool(&app_state).await;

    let planned = run_planned_failover(
        router,
        caller_parts("cross-provider-plan", Some("unit-dev")),
        default_fat_body(),
        pool,
        RequestRequirements::default(),
        None,
    )
    .await
    .expect("openrouter hop must succeed after gemini ladder exhaustion");

    assert_eq!(
        routed_identity(&planned.response),
        format!("openrouter-default/{OPENROUTER_MODEL}"),
        "eight gemini accounts must not block cross-provider failover"
    );
    assert_eq!(
        planned.planned_hops,
        u32::try_from(GEMINI_SLOTS.len() * LADDER_MODELS.len() + 1).unwrap(),
        "plan must include every feasible Gemini account/model plus OpenRouter"
    );
}
