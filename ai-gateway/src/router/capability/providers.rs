use super::ModelCapability;
use crate::types::provider::InferenceProvider;

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
    if model_name.starts_with("openai/") || model_name.starts_with("qwen/") {
        cap.supports_tools = true;
        cap.supports_json_schema = true;
        cap.context_window = Some(128_000);
        cap.supports_vision =
            model_name.contains("gpt-4") || model_name.contains("o1");
    }
}

fn named(cap: &mut ModelCapability, n: &str, model_name: &str) {
    match n {
        "groq" => groq(cap, model_name),
        "mistral" => mistral(cap, model_name),
        "cerebras" => cerebras(cap, model_name),
        "deepseek" => deepseek(cap),
        "xai" => xai(cap, model_name),
        "opencode" => opencode(cap, model_name),
        "cloudflare" => cloudflare(cap, model_name),
        "chatgpt-web" => chatgpt_web(cap),
        _ => {}
    }
}

fn groq(cap: &mut ModelCapability, model_name: &str) {
    cap.supports_tools = true;
    cap.supports_json_schema = groq::supports_json_schema(model_name);
    cap.context_window = Some(8_000);
}

fn mistral(cap: &mut ModelCapability, model_name: &str) {
    cap.supports_tools = true;
    cap.supports_json_schema = mistral::supports_json_schema(model_name);
    cap.context_window = Some(131_072);
    if mistral::supports_reasoning(model_name) {
        cap.reasoning = true;
    }
}

fn cerebras(cap: &mut ModelCapability, model_name: &str) {
    cap.supports_tools = true;
    cap.supports_json_schema = cerebras::supports_json_schema(model_name);
    cap.context_window = Some(131_072);
    if cerebras::supports_reasoning(model_name) {
        cap.reasoning = true;
    }
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

fn opencode(cap: &mut ModelCapability, model_name: &str) {
    cap.supports_tools = true;
    cap.context_window = Some(200_000);
    cap.supports_json_schema = opencode::supports_json_schema(model_name);
    if opencode::supports_reasoning(model_name) {
        cap.reasoning = true;
    }
}

fn cloudflare(cap: &mut ModelCapability, model_name: &str) {
    cap.supports_tools = true;
    cap.context_window = Some(131_072);
    cap.supports_json_schema = cloudflare::supports_json_schema(model_name);
    if cloudflare::supports_reasoning(model_name) {
        cap.reasoning = true;
    }
}

mod groq {
    /// Source: <https://console.groq.com/docs/structured-outputs#supported-models>
    const JSON_SCHEMA_MODELS: &[&str] = &[
        "openai/gpt-oss-120b",
        "openai/gpt-oss-safeguard-20b",
        "meta-llama/llama-4-scout-17b-16e-instruct",
    ];

    pub fn supports_json_schema(model_name: &str) -> bool {
        JSON_SCHEMA_MODELS.iter().any(|m| model_name.contains(m))
    }
}

mod mistral {
    /// Live-probed on La Plateforme (2026-06-13): strict json_schema works.
    const JSON_SCHEMA_MODELS: &[&str] = &[
        "mistral-small",
        "mistral-medium",
        "mistral-large",
        "magistral",
        "codestral",
        "devstral",
        "ministral",
    ];

    const REASONING_MODELS: &[&str] = &["magistral"];

    pub fn supports_json_schema(model_name: &str) -> bool {
        JSON_SCHEMA_MODELS.iter().any(|m| model_name.contains(m))
    }

    pub fn supports_reasoning(model_name: &str) -> bool {
        REASONING_MODELS.iter().any(|m| model_name.contains(m))
    }
}

mod cerebras {
    /// Live-probed + docs (2026-06-13): structured outputs with strict json_schema.
    const JSON_SCHEMA_MODELS: &[&str] = &["gpt-oss-120b", "zai-glm-4.7"];

    const REASONING_MODELS: &[&str] = &["gpt-oss-120b"];

    pub fn supports_json_schema(model_name: &str) -> bool {
        JSON_SCHEMA_MODELS.iter().any(|m| model_name.contains(m))
    }

    pub fn supports_reasoning(model_name: &str) -> bool {
        REASONING_MODELS.iter().any(|m| model_name.contains(m))
    }
}

fn chatgpt_web(cap: &mut ModelCapability) {
    cap.supports_tools = false;
    cap.supports_json_schema = true;
    cap.context_window = Some(128_000);
}

mod opencode {
    /// Verified against https://opencode.ai/zen/v1 on 2026-06-13.
    const JSON_SCHEMA_MODELS: &[&str] =
        &["mimo-v2.5-free", "nemotron-3-ultra-free"];

    /// Models that return `reasoning` / `reasoning_details` on OpenCode Zen.
    const REASONING_MODELS: &[&str] =
        &["nemotron-3-ultra-free", "big-pickle"];

    pub fn supports_json_schema(model_name: &str) -> bool {
        JSON_SCHEMA_MODELS
            .iter()
            .any(|m| model_name.contains(m))
    }

    pub fn supports_reasoning(model_name: &str) -> bool {
        REASONING_MODELS.iter().any(|m| model_name.contains(m))
    }
}

mod cloudflare {
    /// Live-probed on Workers AI OpenAI-compatible API (2026-06-13).
    const JSON_SCHEMA_MODELS: &[&str] = &[
        "llama-3.1-70b-instruct",
        "llama-4-scout-17b-16e-instruct",
        "deepseek-r1-distill-qwen-32b",
        "llama-3.2-3b-instruct",
    ];

    const REASONING_MODELS: &[&str] = &["deepseek-r1-distill-qwen-32b"];

    pub fn supports_json_schema(model_name: &str) -> bool {
        JSON_SCHEMA_MODELS
            .iter()
            .any(|m| model_name.contains(m))
    }

    pub fn supports_reasoning(model_name: &str) -> bool {
        REASONING_MODELS.iter().any(|m| model_name.contains(m))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        router::capability::ModelCapability,
        types::model_id::ModelId,
    };

    #[test]
    fn cerebras_gpt_oss_supports_json_schema_and_reasoning() {
        let mut cap = ModelCapability {
            provider: InferenceProvider::Named("cerebras".into()),
            model: ModelId::from_str_and_provider(
                InferenceProvider::Named("cerebras".into()),
                "gpt-oss-120b",
            )
            .unwrap(),
            context_window: None,
            supports_tools: false,
            supports_json_schema: false,
            supports_vision: false,
            reasoning: false,
        };
        cerebras(&mut cap, "gpt-oss-120b");
        assert!(cap.supports_json_schema);
        assert!(cap.reasoning);
    }

    #[test]
    fn mistral_magistral_supports_json_schema_and_reasoning() {
        let mut cap = ModelCapability {
            provider: InferenceProvider::Named("mistral".into()),
            model: ModelId::from_str_and_provider(
                InferenceProvider::Named("mistral".into()),
                "magistral-medium-latest",
            )
            .unwrap(),
            context_window: None,
            supports_tools: false,
            supports_json_schema: false,
            supports_vision: false,
            reasoning: false,
        };
        mistral(&mut cap, "magistral-medium-latest");
        assert!(cap.supports_json_schema);
        assert!(cap.reasoning);
    }
}
