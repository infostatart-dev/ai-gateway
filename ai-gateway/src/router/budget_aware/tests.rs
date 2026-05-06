use std::time::Duration;

use super::rank::{
    default_budget_rank, default_provider_budget_rank, effective_budget_rank,
};
use crate::{
    router::capability::ModelCapability,
    types::{model_id::ModelId, provider::InferenceProvider},
};

fn capability(provider: InferenceProvider, model: &str) -> ModelCapability {
    let model =
        ModelId::from_str_and_provider(provider.clone(), model).unwrap();
    ModelCapability {
        provider,
        model,
        context_window: None,
        supports_tools: false,
        supports_json_schema: false,
        supports_vision: false,
        reasoning: false,
    }
}

#[test]
fn default_provider_budget_order_matches_autodefault_policy() {
    let groq = InferenceProvider::Named("groq".into());

    assert!(
        default_provider_budget_rank(&InferenceProvider::OpenRouter)
            < default_provider_budget_rank(&groq)
    );
    assert!(
        default_provider_budget_rank(&groq)
            < default_provider_budget_rank(&InferenceProvider::GoogleGemini)
    );
    assert!(
        default_provider_budget_rank(&InferenceProvider::GoogleGemini)
            < default_provider_budget_rank(&InferenceProvider::Anthropic)
    );
    assert!(
        default_provider_budget_rank(&InferenceProvider::Anthropic)
            < default_provider_budget_rank(&InferenceProvider::OpenAI)
    );
}

#[test]
fn ranks_short_cooldown_cheap_provider_before_expensive_provider() {
    let groq = capability(
        InferenceProvider::Named("groq".into()),
        "llama-3.1-8b-instant",
    );
    let anthropic =
        capability(InferenceProvider::Anthropic, "claude-3-7-sonnet");

    assert!(
        effective_budget_rank(
            default_budget_rank(&groq),
            Some(Duration::from_secs(2)),
            Duration::from_secs(3),
        ) < effective_budget_rank(
            default_budget_rank(&anthropic),
            None,
            Duration::from_secs(3),
        )
    );
}
