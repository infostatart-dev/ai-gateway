use std::time::Instant;

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
    assert_eq!(registry.success_rate(&provider, &credential), 1.0);
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
