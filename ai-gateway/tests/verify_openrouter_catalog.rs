//! CI gate: embedded `OpenRouter` wire slugs must exist in frozen `ListModels`
//! fixture.

use std::collections::HashSet;

use ai_gateway::{
    config::{
        catalog_limit_resolve::normalize_model_slug,
        model_ladder::ModelLadderRegistry,
        provider_limits::ProviderLimitCatalog, providers::ProvidersConfig,
    },
    types::provider::InferenceProvider,
};

const FIXTURE: &str = include_str!("fixtures/openrouter-listmodels.json");

fn fixture_ids() -> HashSet<String> {
    let value: serde_json::Value =
        serde_json::from_str(FIXTURE).expect("fixture json");
    let models = value
        .get("data")
        .and_then(|m| m.as_array())
        .expect("data array");
    models
        .iter()
        .filter_map(|entry| entry.get("id").and_then(|id| id.as_str()))
        .map(str::to_string)
        .collect()
}

fn fixture_covers_slug(fixture: &HashSet<String>, slug: &str) -> bool {
    fixture.iter().any(|id| normalize_model_slug(id) == slug)
}

#[test]
fn openrouter_catalog_slugs_exist_in_listmodels_fixture() {
    let fixture = fixture_ids();
    let providers = ProvidersConfig::default();
    let provider = InferenceProvider::OpenRouter;
    let config = providers.get(&provider).expect("openrouter config");
    let mut missing = Vec::new();
    for model in &config.models {
        let slug = model.to_string();
        if !fixture.contains(&slug) {
            missing.push(format!("providers.yaml upstream slug '{slug}'"));
        }
    }
    let ladders = ModelLadderRegistry::default();
    for slug in ladders.ladder_model_slugs(&provider, "free") {
        if !fixture_covers_slug(&fixture, &slug) {
            missing.push(format!("provider-ladders.yaml slug '{slug}'"));
        }
    }
    let limits = ProviderLimitCatalog::default();
    if let Some(tier) = limits.provider(&provider).and_then(|p| p.tier("free"))
    {
        for key in tier.models.keys() {
            if !fixture_covers_slug(&fixture, key) {
                missing
                    .push(format!("provider-limits.yaml per-slug key '{key}'"));
            }
        }
    }
    assert!(
        missing.is_empty(),
        "OpenRouter catalog verify failed — missing from ListModels \
         fixture:\n{}",
        missing.join("\n")
    );
}
