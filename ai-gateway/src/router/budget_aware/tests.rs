use std::time::Duration;

use super::rank::{
    default_budget_rank, default_provider_budget_rank, effective_budget_rank,
};
use crate::{
    router::{
        capability::ModelCapability, provider_attempt::is_failoverable_status,
    },
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
fn payload_too_large_triggers_provider_failover() {
    assert!(is_failoverable_status(http::StatusCode::PAYLOAD_TOO_LARGE));
}

#[test]
fn default_provider_budget_order_matches_autodefault_policy() {
    let opencode = InferenceProvider::Named("opencode".into());
    let mistral = InferenceProvider::Named("mistral".into());
    let groq = InferenceProvider::Named("groq".into());
    let cerebras = InferenceProvider::Named("cerebras".into());
    let cloudflare = InferenceProvider::Named("cloudflare".into());

    assert!(
        default_provider_budget_rank(&opencode)
            < default_provider_budget_rank(&InferenceProvider::OpenRouter)
    );
    assert_eq!(
        default_provider_budget_rank(&InferenceProvider::OpenRouter),
        default_provider_budget_rank(&mistral)
    );
    assert!(
        default_provider_budget_rank(&mistral)
            < default_provider_budget_rank(&groq)
    );
    assert!(
        default_provider_budget_rank(&groq)
            < default_provider_budget_rank(&cerebras)
    );
    assert!(
        default_provider_budget_rank(&cerebras)
            < default_provider_budget_rank(&cloudflare)
    );
    assert!(
        default_provider_budget_rank(&cloudflare)
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
