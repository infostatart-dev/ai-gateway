//! Shared builders for routing-load scenarios (integration tests only).

use ai_gateway::{
    app_state::AppState, config::credentials::ProviderCredentialId,
    metrics::provider::attempt::CallOutcome,
    types::provider::InferenceProvider,
};

pub fn trip_circuit(app_state: &AppState, credential: &str) {
    let cred = ProviderCredentialId::new(credential);
    for _ in 0..5 {
        app_state.credential_health().record_attempt(
            &InferenceProvider::GoogleGemini,
            &cred,
            CallOutcome::RateLimited,
            429,
        );
    }
}

pub fn trip_circuits(app_state: &AppState, credentials: &[&str]) {
    for credential in credentials {
        trip_circuit(app_state, credential);
    }
}
