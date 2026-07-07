use std::time::{Duration, Instant};

use ai_gateway::tests::budget_aware::{
    CallOutcome, CredentialHealthRegistry, InferenceProvider,
    ProviderCredentialId,
};

fn cred(id: &str) -> ProviderCredentialId {
    ProviderCredentialId::new(id)
}

#[test]
fn auth_error_opens_circuit_immediately() {
    let registry = CredentialHealthRegistry::new();
    let provider = InferenceProvider::GoogleGemini;
    let credential = cred("gemini-free-1");
    registry.record_attempt(
        &provider,
        &credential,
        CallOutcome::ClientError,
        401,
    );
    assert!(registry.is_circuit_open(&provider, &credential, Instant::now()));
}

#[test]
fn low_success_rate_opens_circuit_after_min_attempts() {
    let registry = CredentialHealthRegistry::new();
    let provider = InferenceProvider::GoogleGemini;
    let credential = cred("gemini-free-2");
    for _ in 0..5 {
        registry.record_attempt(
            &provider,
            &credential,
            CallOutcome::RateLimited,
            429,
        );
    }
    assert!(registry.is_circuit_open(&provider, &credential, Instant::now()));
    assert!(registry.success_rate(&provider, &credential) < 0.10);
}

#[test]
fn success_closes_circuit() {
    let registry = CredentialHealthRegistry::new();
    let provider = InferenceProvider::GoogleGemini;
    let credential = cred("gemini-free-3");
    for _ in 0..5 {
        registry.record_attempt(
            &provider,
            &credential,
            CallOutcome::RateLimited,
            429,
        );
    }
    registry.record_attempt(&provider, &credential, CallOutcome::Success, 200);
    assert!(!registry.is_circuit_open(&provider, &credential, Instant::now()));
}

#[test]
fn window_rollover_resets_counts() {
    let registry = CredentialHealthRegistry::new();
    let provider = InferenceProvider::GoogleGemini;
    let credential = cred("gemini-free-4");
    registry.testing_seed_stale_window(&provider, &credential, 5, 0);
    registry.record_attempt(&provider, &credential, CallOutcome::Success, 200);
    let success_rate = registry.success_rate(&provider, &credential);
    assert!((success_rate - 1.0).abs() < f64::EPSILON);
}

#[test]
fn credential_zero_success_dead_requires_ten_failures() {
    let registry = CredentialHealthRegistry::new();
    let provider = InferenceProvider::GoogleGemini;
    let credential = cred("gemini-free-dead");
    for _ in 0..5 {
        registry.record_attempt(
            &provider,
            &credential,
            CallOutcome::RateLimited,
            429,
        );
    }
    assert!(!registry.credential_zero_success_dead(
        &provider,
        &credential,
        Instant::now()
    ));
    for _ in 0..5 {
        registry.record_attempt(
            &provider,
            &credential,
            CallOutcome::RateLimited,
            429,
        );
    }
    assert!(registry.credential_zero_success_dead(
        &provider,
        &credential,
        Instant::now()
    ));
}

#[test]
fn model_health_isolated_within_same_credential() {
    let registry = CredentialHealthRegistry::new();
    let provider = InferenceProvider::Named("llm7".into());
    let credential = cred("llm7-default");

    registry.record_model_attempt(
        &provider,
        &credential,
        "gpt-oss:20b",
        CallOutcome::ClientError,
        400,
        Duration::from_millis(80),
    );
    registry.record_model_attempt(
        &provider,
        &credential,
        "fast",
        CallOutcome::Success,
        200,
        Duration::from_millis(40),
    );

    assert_eq!(
        registry.model_success_rate(&provider, &credential, "gpt-oss:20b"),
        0.0
    );
    assert_eq!(
        registry.model_success_rate(&provider, &credential, "fast"),
        1.0
    );
    assert!(
        registry
            .model_latency_ms(&provider, &credential, "fast")
            .is_some()
    );
}
