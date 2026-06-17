use std::time::Duration;

use crate::{
    config::{
        credentials::ProviderCredentialId,
        provider_limits::ProviderLimitCatalog,
    },
    router::pacing::PacingRegistry,
    types::provider::InferenceProvider,
};

pub async fn run() {
    let catalog = ProviderLimitCatalog::default();
    let registry = PacingRegistry::new(catalog);
    let provider = InferenceProvider::Named("groq".into());
    let cred = ProviderCredentialId::new("groq-default");
    let gate = registry
        .gate_for(&provider, Some(&cred))
        .expect("groq pacing gate");
    let _permit = gate.acquire().await.expect("pacing permit");
    // Full min-interval burst behavior is covered by dev/emulated-smoke.sh (12s
    // wait).
    let _ = Duration::from_secs(12);
}
