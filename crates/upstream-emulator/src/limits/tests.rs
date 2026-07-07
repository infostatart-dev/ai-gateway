use ai_gateway::{
    config::provider_limits::ProviderLimitCatalog,
    types::provider::InferenceProvider,
};

use crate::limits::{LimitRegistry, RateLimitVerdict};

#[test]
fn per_credential_rpm_isolation() {
    let catalog = ProviderLimitCatalog::default();
    let registry = LimitRegistry::default();
    let provider = InferenceProvider::Named("groq".into());
    let model = "llama-3.1-8b-instant";
    for _ in 0..30 {
        let _ = registry
            .check_api_key(
                &catalog,
                &provider,
                Some("free"),
                model,
                "key-a",
                10,
            )
            .expect("allow");
    }
    assert!(matches!(
        registry.check_api_key(
            &catalog,
            &provider,
            Some("free"),
            model,
            "key-a",
            10
        ),
        Err(RateLimitVerdict::RpmExceeded)
    ));
    assert!(
        registry
            .check_api_key(
                &catalog,
                &provider,
                Some("free"),
                model,
                "key-b",
                10
            )
            .is_ok()
    );
}

#[test]
fn catalog_rpm_change_changes_enforcement() {
    let mut catalog = ProviderLimitCatalog::default();
    let provider = InferenceProvider::Named("groq".into());
    let entry = catalog.providers.get_mut(&provider).expect("groq");
    let tier = entry.tiers.get_mut("free").expect("free tier");
    let endpoint = tier
        .endpoints
        .get_mut("chat-completions")
        .expect("chat-completions endpoint");
    let model = endpoint
        .models
        .get_mut("llama-3.1-8b-instant")
        .expect("model");
    model.limits.rpm =
        ai_gateway::config::provider_limits::QuotaValue::Limited(2);

    let registry = LimitRegistry::default();
    for _ in 0..2 {
        registry
            .check_api_key(
                &catalog,
                &provider,
                Some("free"),
                "llama-3.1-8b-instant",
                "k",
                1,
            )
            .unwrap();
    }
    assert!(matches!(
        registry.check_api_key(
            &catalog,
            &provider,
            Some("free"),
            "llama-3.1-8b-instant",
            "k",
            1
        ),
        Err(RateLimitVerdict::RpmExceeded)
    ));
}
