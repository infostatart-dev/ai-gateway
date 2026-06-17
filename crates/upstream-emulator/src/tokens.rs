use std::sync::OnceLock;

use serde_json::Value;
use tiktoken_rs::{CoreBPE, o200k_base};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TokenUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
}

impl TokenUsage {
    #[must_use]
    pub fn total(self) -> u32 {
        self.prompt_tokens.saturating_add(self.completion_tokens)
    }
}

fn bpe() -> &'static CoreBPE {
    static BPE: OnceLock<CoreBPE> = OnceLock::new();
    BPE.get_or_init(|| o200k_base().expect("embedded o200k_base"))
}

fn count_tokens(text: &str) -> u32 {
    u32::try_from(bpe().encode_ordinary(text).len()).unwrap_or(u32::MAX)
}

/// Estimate input tokens from the request body using billable text
/// (messages + json_schema description), and completion tokens from content.
#[must_use]
pub fn estimate_usage(body: &Value, content: &str) -> TokenUsage {
    let prompt_tokens = estimate_input(body);
    let completion_tokens = count_tokens(content).max(1);
    TokenUsage {
        prompt_tokens,
        completion_tokens,
    }
}

fn estimate_input(body: &Value) -> u32 {
    let mut text = String::new();
    if let Some(messages) = body.get("messages").and_then(Value::as_array) {
        for msg in messages {
            if let Some(c) = msg.get("content").and_then(Value::as_str) {
                text.push_str(c);
            }
        }
    }
    if let Some(schema) = body
        .pointer("/response_format/json_schema/schema")
        .map(|v| v.to_string())
    {
        text.push_str(&schema);
    }
    if text.is_empty() {
        return 1;
    }
    count_tokens(&text).max(1)
}
