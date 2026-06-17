use serde_json::{Value, json};

use crate::tokens::TokenUsage;

pub fn openai_chat_completion(content: &str, usage: TokenUsage) -> Value {
    json!({
        "id": "emu-completion",
        "object": "chat.completion",
        "created": 1_700_000_000_i64,
        "model": "emulated",
        "choices": [{
            "index": 0,
            "message": { "role": "assistant", "content": content },
            "finish_reason": "stop"
        }],
        "usage": {
            "prompt_tokens": usage.prompt_tokens,
            "completion_tokens": usage.completion_tokens,
            "total_tokens": usage.total()
        }
    })
}

pub fn anthropic_message(content: &str, usage: TokenUsage) -> Value {
    json!({
        "id": "emu-msg",
        "type": "message",
        "role": "assistant",
        "content": [{ "type": "text", "text": content }],
        "model": "emulated",
        "stop_reason": "end_turn",
        "usage": {
            "input_tokens": usage.prompt_tokens,
            "output_tokens": usage.completion_tokens
        }
    })
}

pub fn openai_sse_chunks(content: &str, usage: TokenUsage) -> String {
    let chunk = json!({ "choices": [{ "delta": { "content": content } }] });
    let usage_chunk = json!({
        "choices": [],
        "usage": {
            "prompt_tokens": usage.prompt_tokens,
            "completion_tokens": usage.completion_tokens,
            "total_tokens": usage.total()
        }
    });
    format!("data: {chunk}\n\ndata: {usage_chunk}\n\ndata: [DONE]\n\n")
}
