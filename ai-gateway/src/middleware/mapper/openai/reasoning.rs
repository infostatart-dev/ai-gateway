use async_openai::types::chat::CreateChatCompletionRequest;

/// Source: https://platform.openai.com/docs/guides/reasoning
const PREFIXES: &[&str] = &["o1", "o3", "o4", "gpt-5"];

pub struct ReasoningAdapter;

impl ReasoningAdapter {
    pub fn normalize(req: &mut CreateChatCompletionRequest) {
        if PREFIXES.iter().any(|p| req.model.starts_with(p)) {
            req.temperature = None;
            req.top_p = None;
        }
    }
}
