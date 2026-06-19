use ai_gateway::{
    app_state::AppState,
    config::credentials::ProviderCredentialId,
    metrics::provider::attempt::CallOutcome,
    tests::routing::{
        InferenceProvider, RequestRequirements, clear_test_call_responses,
        empty_router, gemini_model_candidate, install_upstream_mock,
    },
};
use gateway_tests::{UpstreamMockScript, upstream::rate_limited_rpm};

use crate::rl::support::*;

const MODEL: &str = "gemini-3.1-flash-lite";
const CRED_EIGHT: &str = "gemini-free-8";
const CRED_NINE: &str = "gemini-free-9";

pub async fn run() {
    clear_test_call_responses();
    install_upstream_mock(
        UpstreamMockScript::new()
            .credential(CRED_EIGHT, vec![rate_limited_rpm; 8])
            .credential(
                CRED_NINE,
                vec![gateway_tests::upstream::ok_chat_completion],
            ),
    );

    let app_state = AppState::test_default().await;
    let cred = ProviderCredentialId::new(CRED_EIGHT);
    for _ in 0..5 {
        app_state.credential_health().record_attempt(
            &InferenceProvider::GoogleGemini,
            &cred,
            CallOutcome::RateLimited,
            429,
        );
    }

    let pool = || async {
        vec![
            gemini_model_candidate(&app_state, CRED_EIGHT, MODEL).await,
            gemini_model_candidate(&app_state, CRED_NINE, MODEL).await,
        ]
    };

    for i in 0..3 {
        let _ = run_planned_failover(
            empty_router(&app_state),
            caller_parts("warmup", Some(&format!("warmup-{i}"))),
            default_fat_body(),
            pool().await,
            RequestRequirements::default(),
            None,
        )
        .await;
    }

    let before = credential_attempts(&app_state, CRED_EIGHT);
    run_planned_failover(
        empty_router(&app_state),
        caller_parts("follow-up", Some("unit-new")),
        default_fat_body(),
        pool().await,
        RequestRequirements::default(),
        None,
    )
    .await
    .expect("healthy slot succeeds");

    assert_eq!(
        credential_attempts(&app_state, CRED_EIGHT),
        before,
        "circuit-open credential must not receive follow-up attempts"
    );
}
