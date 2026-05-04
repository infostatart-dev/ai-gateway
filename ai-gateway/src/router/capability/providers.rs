use crate::types::provider::InferenceProvider;

use super::ModelCapability;

pub fn apply_provider_capabilities(
    cap: &mut ModelCapability,
    provider: &InferenceProvider,
    model_name: &str,
) {
    match provider {
        InferenceProvider::OpenAI => openai(cap, model_name),
        InferenceProvider::Anthropic => anthropic(cap),
        InferenceProvider::GoogleGemini => gemini(cap),
        InferenceProvider::OpenRouter => openrouter(cap, model_name),
        InferenceProvider::Named(n) => named(cap, n, model_name),
        _ => {}
    }
}

fn openai(cap: &mut ModelCapability, model_name: &str) {
    cap.supports_tools = true;
    cap.supports_json_schema = true;
    cap.context_window = Some(128_000);
    cap.supports_vision = model_name.contains("vision")
        || model_name.contains("-4o")
        || model_name.contains("o1");
}

fn anthropic(cap: &mut ModelCapability) {
    cap.supports_tools = true;
    cap.supports_json_schema = true;
    cap.supports_vision = true;
    cap.context_window = Some(200_000);
}

fn gemini(cap: &mut ModelCapability) {
    cap.supports_tools = true;
    cap.supports_json_schema = true;
    cap.supports_vision = true;
    cap.context_window = Some(1_000_000);
}

fn openrouter(cap: &mut ModelCapability, model_name: &str) {
    if !model_name.starts_with("openai/") {
        return;
    }
    cap.supports_tools = true;
    cap.supports_json_schema = true;
    cap.context_window = Some(128_000);
    cap.supports_vision =
        model_name.contains("gpt-4") || model_name.contains("o1");
}

fn named(cap: &mut ModelCapability, n: &str, model_name: &str) {
    match n {
        "groq" => groq(cap, model_name),
        "deepseek" => deepseek(cap),
        "xai" => xai(cap, model_name),
        _ => {}
    }
}

fn groq(cap: &mut ModelCapability, model_name: &str) {
    cap.supports_tools = true;
    cap.supports_json_schema = groq::supports_json_schema(model_name);
    cap.context_window = Some(8_000);
}

fn deepseek(cap: &mut ModelCapability) {
    cap.supports_tools = true;
    cap.supports_json_schema = true;
    cap.context_window = Some(8_000);
}

fn xai(cap: &mut ModelCapability, model_name: &str) {
    cap.supports_tools = true;
    cap.supports_json_schema = true;
    cap.context_window = Some(8_000);
    if model_name.contains("vision") {
        cap.supports_vision = true;
    }
}

mod groq {
    /// Source: https://console.groq.com/docs/structured-outputs#supported-models
    const JSON_SCHEMA_MODELS: &[&str] = &[
        "openai/gpt-oss-120b",
        "openai/gpt-oss-safeguard-20b",
        "meta-llama/llama-4-scout-17b-16e-instruct",
    ];

    pub fn supports_json_schema(model_name: &str) -> bool {
        JSON_SCHEMA_MODELS.iter().any(|m| model_name.contains(m))
    }
}
