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
    if capability.provider == InferenceProvider::OpenRouter
        && capability.model.to_string().ends_with(":free")
    {
        return 0;
    }
    default_provider_budget_rank(&capability.provider)
}

/// Provider priority within a cost-class band (lower = tried first).
/// Order: opencode → openrouter → github-models → mistral → groq → cerebras →
/// cloudflare → gemini → deepseek-web → anthropic → openai → chatgpt-web.
#[allow(clippy::match_same_arms)]
pub(crate) fn default_provider_budget_rank(
    provider: &InferenceProvider,
) -> u16 {
    match provider {
        InferenceProvider::Named(name) if name == "opencode" => 0,
        InferenceProvider::Ollama | InferenceProvider::OpenRouter => 1,
        InferenceProvider::Named(name) if name == "github-models" => 2,
        InferenceProvider::Named(name) if name == "mistral" => 3,
        InferenceProvider::Named(name) if name == "groq" => 4,
        InferenceProvider::Named(name) if name == "cerebras" => 5,
        InferenceProvider::Named(name) if name == "cloudflare" => 6,
        InferenceProvider::GoogleGemini => 7,
        InferenceProvider::Named(name) if name == "deepseek-web" => 8,
        InferenceProvider::Anthropic => 9,
        InferenceProvider::OpenAI => 10,
        InferenceProvider::Named(name) if name == "chatgpt-web" => 11,
        InferenceProvider::Named(name)
            if matches!(
                name.as_str(),
                "bazaarlink"
                    | "bluesminds"
                    | "sambanova"
                    | "inclusionai"
                    | "ollama-cloud"
                    | "cohere"
                    | "doubao"
            ) =>
        {
            20
        }
        InferenceProvider::Named(name) if name == "longcat" => 30,
        InferenceProvider::Named(name) if name == "deepseek" => 10,
        InferenceProvider::Bedrock => 50,
        InferenceProvider::Named(_) => 25,
    }
}
