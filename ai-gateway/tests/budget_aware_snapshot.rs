use std::time::{Duration, Instant};

use ai_gateway::{
    app_state::AppState,
    tests::budget_aware::{
        CallOutcome, CredentialHealthRegistry, InferenceProvider, PacingGate,
        PacingLimits, PacingRegistry, ProviderCredentialId, QuotaSnapshot,
        empty_router, gemini_model_candidate,
    },
};

#[tokio::test]
async fn rpd_zero_yields_zero_headroom() {
    let gate = PacingGate::new(PacingLimits {
        concurrent: 4,
        rpm: u32::MAX,
        tpm: None,
        rpd: Some(1),
        tpd: None,
        daily_reset_utc_hour: 0,
        min_interval: Duration::ZERO,
        max_queue_wait: Duration::from_secs(1),
    });
    gate.acquire(0).await.unwrap();
    assert!(!gate.daily_headroom_available(0).await);
}

#[tokio::test]
async fn rpm_available_yields_positive_headroom() {
    let app_state = AppState::test_default().await;
    let candidate = gemini_model_candidate(
        &app_state,
        "gemini-free-1",
        "gemini-2.5-flash-lite",
    )
    .await;
    let health = CredentialHealthRegistry::new();
    let snapshot = QuotaSnapshot::capture(
        app_state.upstream_pacing(),
        &health,
        &empty_router(&app_state),
        std::slice::from_ref(&candidate),
        100,
        Duration::from_secs(3),
        Instant::now(),
    )
    .await;
    assert!(
        snapshot.headroom_score("gemini-free-1", "gemini-2.5-flash-lite") > 0.0
    );
}

#[tokio::test]
async fn catalog_slug_normalization_matches_snapshot_key() {
    let app_state = AppState::test_default().await;
    let candidate = gemini_model_candidate(
        &app_state,
        "gemini-free-2",
        "models/gemini-2.5-flash-lite",
    )
    .await;
    let health = CredentialHealthRegistry::new();
    health.record_attempt(
        &InferenceProvider::GoogleGemini,
        &ProviderCredentialId::new("gemini-free-2"),
        CallOutcome::RateLimited,
        429,
    );
    let registry =
        PacingRegistry::new(app_state.config().provider_limits.clone());
    let snapshot = QuotaSnapshot::capture(
        &registry,
        &health,
        &empty_router(&app_state),
        std::slice::from_ref(&candidate),
        0,
        Duration::from_secs(3),
        Instant::now(),
    )
    .await;
    assert!(
        snapshot
            .headroom_score("gemini-free-2", "models/gemini-2.5-flash-lite")
            >= 0.0
    );
}
