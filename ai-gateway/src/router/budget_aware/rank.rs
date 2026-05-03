use std::time::Duration;

use crate::{
    router::capability::ModelCapability, types::provider::InferenceProvider,
};

pub(super) fn effective_budget_rank(
    base_rank: u16,
    remaining_cooldown: Option<Duration>,
    max_cooldown_wait: Duration,
) -> u16 {
    base_rank
        .saturating_mul(10)
        .saturating_add(remaining_cooldown.map_or(0, |remaining| {
            if remaining <= max_cooldown_wait {
                5
            } else {
                1_000
            }
        }))
}

pub(super) fn default_budget_rank(capability: &ModelCapability) -> u16 {
    match &capability.provider {
        InferenceProvider::Ollama => 0,
        InferenceProvider::Named(name) if name == "groq" => 0,
        InferenceProvider::GoogleGemini => 1,
        InferenceProvider::Named(name) if name == "deepseek" => 10,
        InferenceProvider::OpenRouter
            if capability.model.to_string().ends_with(":free") =>
        {
            0
        }
        InferenceProvider::OpenRouter => 20,
        InferenceProvider::OpenAI => 30,
        InferenceProvider::Anthropic => 40,
        InferenceProvider::Bedrock => 50,
        InferenceProvider::Named(_) => 25,
    }
}
