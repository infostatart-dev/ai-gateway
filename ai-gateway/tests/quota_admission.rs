use std::time::{Duration, Instant};

use ai_gateway::{
    app_state::AppState,
    tests::budget_aware::{
        BlockedReason, CredentialHealthRegistry, InferenceProvider,
        PacingAdmissionScope, PacingGate, PacingLimits, QuotaSnapshot,
        empty_router, evaluate_candidate, evaluate_pacing_admission,
        gemini_model_candidate,
    },
};

#[tokio::test]
async fn admission_infeasible_after_upstream_reconcile() {
    let app_state = AppState::test_default().await;
    let candidate = gemini_model_candidate(
        &app_state,
        "gemini-free-2",
        "gemini-2.5-flash-lite",
    )
    .await;
    let registry = app_state.upstream_pacing();
    let gate = registry
        .gate_for(
            &candidate.capability.provider,
            Some(&candidate.credential_id),
            Some(candidate.credential_tier.as_str()),
            Some(&candidate.capability.model.to_string()),
        )
        .expect("gate");
    gate.apply_upstream_reconcile(Instant::now() + Duration::from_secs(120))
        .await;
    let health = CredentialHealthRegistry::new();
    let router = empty_router(&app_state);

    let verdict = evaluate_candidate(
        registry,
        &health,
        &app_state.config().provider_limits,
        &router,
        &candidate,
        0,
        Instant::now(),
    )
    .await;

    assert!(!verdict.feasible);
    assert_eq!(verdict.blocked_reason, BlockedReason::UpstreamReconcile);
}

#[tokio::test]
async fn per_model_admission_feasible_when_pacing_clear() {
    let app_state = AppState::test_default().await;
    let candidate = gemini_model_candidate(
        &app_state,
        "gemini-free-1",
        "gemini-2.5-flash-lite",
    )
    .await;
    let health = CredentialHealthRegistry::new();
    let router = empty_router(&app_state);

    let verdict = evaluate_candidate(
        app_state.upstream_pacing(),
        &health,
        &app_state.config().provider_limits,
        &router,
        &candidate,
        100,
        Instant::now(),
    )
    .await;

    assert!(verdict.feasible);
    assert_eq!(verdict.blocked_reason, BlockedReason::None);
}

#[tokio::test]
async fn snapshot_capture_matches_direct_admission_verdict() {
    let app_state = AppState::test_default().await;
    let candidate = gemini_model_candidate(
        &app_state,
        "gemini-free-4",
        "gemini-2.5-flash-lite",
    )
    .await;
    let gate = app_state
        .upstream_pacing()
        .gate_for(
            &candidate.capability.provider,
            Some(&candidate.credential_id),
            Some(candidate.credential_tier.as_str()),
            Some(&candidate.capability.model.to_string()),
        )
        .expect("gate");
    for _ in 0..15 {
        let _permit = gate.acquire(0).await.unwrap();
    }
    let health = CredentialHealthRegistry::new();
    let router = empty_router(&app_state);
    let now = Instant::now();
    let verdict = evaluate_candidate(
        app_state.upstream_pacing(),
        &health,
        &app_state.config().provider_limits,
        &router,
        &candidate,
        0,
        now,
    )
    .await;
    let snapshot = QuotaSnapshot::capture(
        app_state.upstream_pacing(),
        &health,
        &router,
        std::slice::from_ref(&candidate),
        0,
        Duration::from_secs(3),
        now,
    )
    .await;
    assert_eq!(
        snapshot.blocked_reason("gemini-free-4", "gemini-2.5-flash-lite"),
        verdict.blocked_reason
    );
    assert_eq!(
        snapshot.headroom_score("gemini-free-4", "gemini-2.5-flash-lite"),
        verdict.headroom_score()
    );
    assert!(
        snapshot
            .next_available_at("gemini-free-4", "gemini-2.5-flash-lite")
            .is_some()
    );
    assert!(verdict.next_available_at.is_some());
}

#[tokio::test]
async fn strict_admission_zero_headroom_on_subsecond_rpm_wait() {
    let app_state = AppState::test_default().await;
    let candidate = gemini_model_candidate(
        &app_state,
        "gemini-free-3",
        "gemini-2.5-flash-lite",
    )
    .await;
    let gate = app_state
        .upstream_pacing()
        .gate_for(
            &candidate.capability.provider,
            Some(&candidate.credential_id),
            Some(candidate.credential_tier.as_str()),
            Some(&candidate.capability.model.to_string()),
        )
        .expect("gate");
    for _ in 0..15 {
        let _permit = gate.acquire(0).await.unwrap();
    }
    let health = CredentialHealthRegistry::new();
    let router = empty_router(&app_state);

    let snapshot = QuotaSnapshot::capture(
        app_state.upstream_pacing(),
        &health,
        &router,
        std::slice::from_ref(&candidate),
        0,
        Duration::from_secs(3),
        Instant::now(),
    )
    .await;

    assert_eq!(
        snapshot.headroom_score("gemini-free-3", "gemini-2.5-flash-lite"),
        0.0,
        "rpm-saturated model scope must have zero headroom"
    );
}

#[tokio::test]
async fn upstream_reconcile_blocks_admission() {
    let gate = PacingGate::new(PacingLimits {
        concurrent: 4,
        rpm: u32::MAX,
        tpm: None,
        rpd: None,
        tpd: None,
        daily_reset_utc_hour: 0,
        min_interval: Duration::ZERO,
        max_queue_wait: Duration::from_secs(1),
    });
    gate.apply_upstream_reconcile(Instant::now() + Duration::from_secs(30))
        .await;
    let wait = gate.upstream_reconcile_wait(Instant::now()).await;
    assert!(wait > Duration::from_secs(25));
}

#[tokio::test]
async fn per_slot_admission_blocks_saturated_gate() {
    use ai_gateway::{
        config::credentials::ProviderCredentialId,
        types::provider::InferenceProvider,
    };

    let app_state = AppState::test_default().await;
    let provider = InferenceProvider::Named("github-models".into());
    let credential = ProviderCredentialId::new("github-models-default");
    let pacing = app_state.upstream_pacing();
    let gate = pacing
        .gate_for(&provider, Some(&credential), Some("free"), None)
        .expect("per-slot gate");
    gate.apply_upstream_reconcile(Instant::now() + Duration::from_secs(60))
        .await;
    let health = CredentialHealthRegistry::new();
    let limits = &app_state.config().provider_limits;
    let verdict = evaluate_pacing_admission(PacingAdmissionScope {
        pacing,
        health: &health,
        limits,
        provider: &provider,
        credential_id: &credential,
        tier: "free",
        model: None,
        estimated_tokens: 0,
        now: Instant::now(),
    })
    .await;
    assert!(!verdict.feasible);
    assert_eq!(verdict.blocked_reason, BlockedReason::UpstreamReconcile);
}

#[tokio::test]
async fn per_session_chatgpt_admission_blocks_saturated_gate() {
    use ai_gateway::{
        config::credentials::ProviderCredentialId,
        types::provider::InferenceProvider,
    };

    let app_state = AppState::test_default().await;
    let provider = InferenceProvider::Named("chatgpt-web".into());
    let credential = ProviderCredentialId::new("chatgpt-web-default");
    let pacing = app_state.upstream_pacing();
    let gate = pacing
        .gate_for(&provider, Some(&credential), Some("session"), None)
        .expect("per-session gate");
    let _hold = gate.acquire(0).await.unwrap();
    let health = CredentialHealthRegistry::new();
    let limits = &app_state.config().provider_limits;
    let verdict = evaluate_pacing_admission(PacingAdmissionScope {
        pacing,
        health: &health,
        limits,
        provider: &provider,
        credential_id: &credential,
        tier: "session",
        model: None,
        estimated_tokens: 0,
        now: Instant::now(),
    })
    .await;
    assert!(!verdict.feasible);
}

#[tokio::test]
#[serial_test::serial]
async fn per_session_deepseek_credentials_admit_independently() {
    use ai_gateway::{
        config::{
            credentials::ProviderCredentialId, secrets_file::SecretsFile,
        },
        types::provider::InferenceProvider,
    };

    const MODEL: &str = "deepseek-chat";
    const BLOCKED: &str = "deepseek-web-default";
    const SIBLING: &str = "deepseek-web-2";

    let dir = std::env::temp_dir()
        .join(format!("ai-gw-ds-admit-unit-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("session dir");
    let first = dir.join("deepseek-a.json");
    let second = dir.join("deepseek-b.json");
    std::fs::write(&first, r#"{"token":"user-session-token"}"#)
        .expect("session");
    std::fs::write(&second, r#"{"token":"sibling-session-token"}"#)
        .expect("session");
    let secrets_path = dir.join("secrets.yaml");
    std::fs::write(&secrets_path, "credentials: {}\nintegrations: {}\n")
        .expect("secrets yaml");
    let mut secrets = SecretsFile::load(&secrets_path).expect("load secrets");
    secrets.register_session_path(BLOCKED, first.clone());
    secrets.register_session_path(SIBLING, second.clone());
    assert_ne!(
        first, second,
        "distinct session files must not share pacing scope"
    );

    let app_state = AppState::test_default().await;
    let _guard = SecretsFile::install_for_test(secrets);

    let provider = InferenceProvider::Named("deepseek-web".into());
    let blocked = ProviderCredentialId::new(BLOCKED);
    let sibling = ProviderCredentialId::new(SIBLING);
    let pacing = app_state.upstream_pacing();
    let gate = pacing
        .gate_for(&provider, Some(&blocked), Some("free"), Some(MODEL))
        .expect("blocked session gate");
    gate.apply_upstream_reconcile(Instant::now() + Duration::from_secs(90))
        .await;

    let health = CredentialHealthRegistry::new();
    let limits = &app_state.config().provider_limits;
    let blocked_verdict = evaluate_pacing_admission(PacingAdmissionScope {
        pacing,
        health: &health,
        limits,
        provider: &provider,
        credential_id: &blocked,
        tier: "free",
        model: Some(MODEL),
        estimated_tokens: 0,
        now: Instant::now(),
    })
    .await;
    assert!(!blocked_verdict.feasible);

    let sibling_verdict = evaluate_pacing_admission(PacingAdmissionScope {
        pacing,
        health: &health,
        limits,
        provider: &provider,
        credential_id: &sibling,
        tier: "free",
        model: Some(MODEL),
        estimated_tokens: 0,
        now: Instant::now(),
    })
    .await;
    assert!(sibling_verdict.feasible);

    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn per_model_credentials_admit_independently() {
    use ai_gateway::config::credentials::ProviderCredentialId;

    let app_state = AppState::test_default().await;
    let provider = InferenceProvider::GoogleGemini;
    let pacing = app_state.upstream_pacing();
    let health = CredentialHealthRegistry::new();
    let limits = &app_state.config().provider_limits;
    let model = "gemini-2.5-flash-lite";
    let blocked = ProviderCredentialId::new("gemini-free-2");
    let sibling = ProviderCredentialId::new("gemini-free-3");
    let gate = pacing
        .gate_for(&provider, Some(&blocked), Some("free"), Some(model))
        .expect("model gate");
    gate.apply_upstream_reconcile(Instant::now() + Duration::from_secs(90))
        .await;

    let blocked_verdict = evaluate_pacing_admission(PacingAdmissionScope {
        pacing,
        health: &health,
        limits,
        provider: &provider,
        credential_id: &blocked,
        tier: "free",
        model: Some(model),
        estimated_tokens: 0,
        now: Instant::now(),
    })
    .await;
    assert!(!blocked_verdict.feasible);

    let sibling_verdict = evaluate_pacing_admission(PacingAdmissionScope {
        pacing,
        health: &health,
        limits,
        provider: &provider,
        credential_id: &sibling,
        tier: "free",
        model: Some(model),
        estimated_tokens: 0,
        now: Instant::now(),
    })
    .await;
    assert!(sibling_verdict.feasible);
}
