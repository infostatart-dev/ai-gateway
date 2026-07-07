//! CI gate: embedded Gemini upstream slugs must exist in frozen `ListModels`
//! fixture.

use std::collections::HashSet;

use ai_gateway::{
    config::{model_ladder::ModelLadderRegistry, providers::ProvidersConfig},
    types::provider::InferenceProvider,
};

const FIXTURE: &str = include_str!("fixtures/gemini-listmodels.json");

fn fixture_slugs() -> HashSet<String> {
    let value: serde_json::Value =
        serde_json::from_str(FIXTURE).expect("fixture json");
    let models = value
        .get("models")
        .and_then(|m| m.as_array())
        .expect("models array");
    models
        .iter()
        .filter_map(|entry| {
            let name = entry.get("name")?.as_str()?;
            let methods = entry
                .get("supportedGenerationMethods")
                .and_then(|m| m.as_array())?;
            if !methods
                .iter()
                .any(|m| m.as_str() == Some("generateContent"))
            {
                return None;
            }
            let slug = name.strip_prefix("models/").unwrap_or(name);
            Some(slug.to_string())
        })
        .collect()
}

#[test]
fn gemini_catalog_slugs_exist_in_listmodels_fixture() {
    let fixture = fixture_slugs();
    let providers = ProvidersConfig::default();
    let provider = InferenceProvider::GoogleGemini;
    let config = providers.get(&provider).expect("gemini config");
    let mut missing = Vec::new();
    for model in &config.models {
        let slug = model.to_string();
        if !fixture.contains(&slug) {
            missing.push(format!("providers.yaml upstream slug '{slug}'"));
        }
    }
    let ladders = ModelLadderRegistry::default();
    for slug in ladders.ladder_model_slugs(&provider, "free") {
        if !fixture.contains(&slug) {
            missing.push(format!("provider-ladders.yaml slug '{slug}'"));
        }
    }
    assert!(
        missing.is_empty(),
        "Gemini catalog verify failed — missing from ListModels fixture:\n{}",
        missing.join("\n")
    );
}
