use std::time::Duration;

use crate::{
    config::provider_limits::ProviderLimitCatalog,
    router::pacing::PacingRegistry, types::provider::InferenceProvider,
};

pub async fn run() {
    let registry = PacingRegistry::new(ProviderLimitCatalog::default());
    let provider = InferenceProvider::Named("chatgpt-web".into());
    let gate = registry
        .gate_for(&provider, None)
        .expect("chatgpt-web pacing gate");
    let first = gate.acquire().await.expect("first permit");
    let second =
        tokio::time::timeout(Duration::from_millis(50), gate.acquire()).await;
    assert!(second.is_err(), "second concurrent acquire should wait");
    drop(first);
    gate.acquire().await.expect("second permit after release");
}
