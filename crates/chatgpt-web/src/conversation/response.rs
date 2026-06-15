use serde_json::{Value, json};

pub fn build_non_streaming_response(model: &str, content: &str) -> Value {
    let created = chrono::Utc::now().timestamp();
    let prompt_tokens = (content.len() / 4).max(1) as u32;
    let completion_tokens = (content.len() / 4).max(1) as u32;
    json!({
        "id": format!("chatcmpl-{}", uuid::Uuid::new_v4()),
        "object": "chat.completion",
        "created": created,
        "model": model,
        "system_fingerprint": null,
        "choices": [{
            "index": 0,
            "message": { "role": "assistant", "content": content },
            "finish_reason": "stop",
            "logprobs": null,
        }],
        "usage": {
            "prompt_tokens": prompt_tokens,
            "completion_tokens": completion_tokens,
            "total_tokens": prompt_tokens + completion_tokens,
        }
    })
}

pub fn content_is_valid_json(response: &Value) -> bool {
    let Some(content) = response
        .pointer("/choices/0/message/content")
        .and_then(Value::as_str)
    else {
        return false;
    };
    if content.trim().is_empty() {
        return false;
    }
    serde_json::from_str::<Value>(content.trim()).is_ok()
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn assistant_response(content: &str) -> Value {
        json!({
            "choices": [{ "message": { "content": content } }]
        })
    }

    #[test]
    fn accepts_raw_json_object() {
        assert!(content_is_valid_json(&assistant_response(
            r#"{"name":"Ada"}"#
        )));
    }

    #[test]
    fn rejects_prose_wrapped_json() {
        assert!(!content_is_valid_json(&assistant_response(
            "Here is the result: {\"name\":\"Ada\"}"
        )));
    }

    #[test]
    fn rejects_markdown_table() {
        assert!(!content_is_valid_json(&assistant_response(
            "| name |\n| --- |\n| Ada |"
        )));
    }

    #[test]
    fn rejects_fenced_json_because_only_raw_json_is_allowed() {
        assert!(!content_is_valid_json(&assistant_response(
            "```json\n{\"ok\":true}\n```"
        )));
    }

    #[test]
    fn rejects_truncated_json() {
        assert!(!content_is_valid_json(&assistant_response(r#"{"ok":true"#)));
    }
}
