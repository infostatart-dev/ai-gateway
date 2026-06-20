use std::collections::HashSet;

use ai_gateway::{
    app_state::AppState,
    tests::routing::{
        RequestRequirements, clear_test_call_responses, empty_router,
        gemini_model_candidate, install_upstream_mock,
    },
};
use futures::future::join_all;
use gateway_tests::{UpstreamMockScript, upstream::ok_chat_completion};

use crate::rl::support::*;

const MODEL: &str = "gemini-3.1-flash-lite";
const SLOT_COUNT: usize = 16;
const WORK_UNITS: usize = 8;

fn gemini_cred(index: usize) -> String {
    if index == 1 {
        "gemini-free".into()
    } else {
        format!("gemini-free-{index}")
    }
}

pub async fn run() {
    clear_test_call_responses();
    install_upstream_mock(
        UpstreamMockScript::new().default_response(ok_chat_completion),
    );

    let app_state = AppState::test_default().await;
    let router = empty_router(&app_state);
    let mut pool = Vec::with_capacity(SLOT_COUNT);
    for index in 1..=SLOT_COUNT {
        pool.push(
            gemini_model_candidate(&app_state, &gemini_cred(index), MODEL)
                .await,
        );
    }
    let body = default_fat_body();

    let futures = (1..=WORK_UNITS).map(|unit| {
        let router = router.clone();
        let pool = pool.clone();
        let body = body.clone();
        async move {
            run_planned_failover(
                router,
                caller_parts(
                    &format!("admission-spread-{unit}"),
                    Some(&format!("unit-{unit}")),
                ),
                body,
                pool,
                RequestRequirements::default(),
                None,
            )
            .await
            .expect("feasible spread route")
        }
    });
    let results = join_all(futures).await;

    let mut accounts = HashSet::new();
    for result in &results {
        let identity = routed_identity(&result.response);
        let cred = identity.split('/').next().unwrap_or_default();
        accounts.insert(cred.to_string());
    }
    assert!(
        accounts.len() >= WORK_UNITS,
        "expected at least {WORK_UNITS} distinct first-hop accounts, got \
         {accounts:?}"
    );
    assert_eq!(
        app_state
            .provider_stats_snapshot(None, None)
            .routing
            .repeat_429_violations,
        0
    );
}
