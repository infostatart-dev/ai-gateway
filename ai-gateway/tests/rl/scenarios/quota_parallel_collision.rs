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

use crate::rl::support::*;

const MODEL: &str = "gemini-3.1-flash-lite";
const HEADROOM: [&str; 2] = ["gemini-free-9", "gemini-free-10"];
/// Catalog RPM for flash-lite free tier.
const FLASH_LITE_RPM: u32 = 15;

fn collision_script() -> UpstreamMockScript {
    let mut script = UpstreamMockScript::new();
    for cred in HEADROOM {
        script = script.binding(cred, MODEL, vec![ok_chat_completion]);
    }
    script
        .binding("gemini-free-11", MODEL, vec![ok_chat_completion])
        .binding(
            "openrouter-default",
            "openai/gpt-oss-120b:free",
            vec![ok_chat_completion],
        )
        .default_response(ok_chat_completion)
}

async fn saturate_non_headroom(app_state: &AppState) {
    let saturated = [
        "gemini-free",
        "gemini-free-2",
        "gemini-free-3",
        "gemini-free-4",
        "gemini-free-5",
        "gemini-free-6",
        "gemini-free-7",
        "gemini-free-8",
        "gemini-free-11",
        "gemini-free-12",
        "gemini-free-13",
        "gemini-free-14",
        "gemini-free-15",
        "gemini-free-16",
    ];
    for cred in saturated {
        saturate_model_pacing(app_state, cred, MODEL, FLASH_LITE_RPM).await;
    }
}

pub async fn run() {
    clear_test_call_responses();
    install_upstream_mock(collision_script());

    let app_state = AppState::test_default().await;
    saturate_non_headroom(&app_state).await;

    let router = empty_router(&app_state);
    let pool = vec![
        gemini_model_candidate(&app_state, HEADROOM[0], MODEL).await,
        gemini_model_candidate(&app_state, HEADROOM[1], MODEL).await,
        gemini_model_candidate(&app_state, "gemini-free-11", MODEL).await,
        openrouter_model_candidate(
            &app_state,
            "openrouter-default",
            "openai/gpt-oss-120b:free",
        )
        .await,
    ];
    let body = default_fat_body();

    let futures = (1..=3).map(|unit| {
        let router = router.clone();
        let pool = pool.clone();
        let body = body.clone();
        async move {
            run_planned_failover(
                router,
                caller_parts("collision", Some(&format!("unit-{unit}"))),
                body,
                pool,
                RequestRequirements::default(),
                None,
            )
            .await
            .expect("collision route")
        }
    });
    let results = join_all(futures).await;

    let identities: Vec<_> = results
        .iter()
        .map(|r| routed_identity(&r.response))
        .collect();
    assert_eq!(identities.len(), 3);
    assert!(
        identities.iter().all(|id| {
            id.starts_with("gemini-free-9/")
                || id.starts_with("gemini-free-10/")
                || id.contains("openrouter")
        }),
        "routes must stay on headroom keys or openrouter, got {identities:?}"
    );
    assert!(
        !identities
            .iter()
            .any(|id| id.starts_with("gemini-free-11/")),
        "saturated credential must not be selected, got {identities:?}"
    );
}
