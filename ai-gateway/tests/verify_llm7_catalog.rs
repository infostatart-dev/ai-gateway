//! CI gate: embedded LLM7 concrete free-token slugs must exist in the frozen
//! live Models API fixture. Selector models are intentionally allowed.

use std::collections::HashSet;

use ai_gateway::{
    config::{
        provider_limits::ProviderLimitCatalog, providers::ProvidersConfig,
    },
    types::provider::InferenceProvider,
};

const FIXTURE: &str = include_str!("fixtures/llm7-listmodels.json");

fn fixture_turbo_ids() -> HashSet<String> {
    let value: serde_json::Value =
        serde_json::from_str(FIXTURE).expect("fixture json");
    let models = value
        .get("data")
        .and_then(|m| m.as_array())
        .expect("data array");
    models
        .iter()
        .filter(|entry| {
            entry.get("tier").and_then(|id| id.as_str()) == Some("turbo")
        })
        .filter_map(|entry| entry.get("id").and_then(|id| id.as_str()))
        .map(str::to_string)
        .collect()
}

#[test]
fn llm7_catalog_slugs_exist_in_listmodels_fixture() {
    let fixture = fixture_turbo_ids();
    let providers = ProvidersConfig::default();
    let provider = InferenceProvider::Named("llm7".into());
    let config = providers.get(&provider).expect("llm7 config");
    let mut missing = Vec::new();
    for model in &config.models {
        let slug = model.to_string();
        if matches!(slug.as_str(), "default" | "fast") {
            continue;
        }
        if !fixture.contains(&slug) {
            missing.push(format!("providers.yaml upstream slug '{slug}'"));
        }
    }
    assert!(
        missing.is_empty(),
        "LLM7 catalog verify failed; missing from turbo Models API \
         fixture:\n{}",
        missing.join("\n")
    );
}

#[test]
fn llm7_is_api_key_scoped_not_browser_session() {
    let limits = ProviderLimitCatalog::default();
    let provider = InferenceProvider::Named("llm7".into());
    let entry = limits.provider(&provider).expect("llm7 limits");
    assert_eq!(entry.scope.as_deref(), Some("api-key"));
}
