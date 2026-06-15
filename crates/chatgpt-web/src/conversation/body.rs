use serde_json::Value;

#[derive(Debug, Clone, Default)]
pub struct ParsedMessages {
    pub system_msg: String,
    pub history: Vec<(String, String)>,
    pub current_msg: String,
}

pub fn parse_openai_messages(messages: &[Value]) -> ParsedMessages {
    let mut system_msg = String::new();
    let mut history = Vec::new();

    for msg in messages {
        let mut role = msg
            .get("role")
            .and_then(Value::as_str)
            .unwrap_or("user")
            .to_string();
        if role == "developer" {
            role = "system".into();
        }
        let content = message_content(msg);
        if content.trim().is_empty() {
            continue;
        }
        if role == "system" {
            if !system_msg.is_empty() {
                system_msg.push('\n');
            }
            system_msg.push_str(&content);
        } else if role == "user" || role == "assistant" {
            history.push((role, content));
        }
    }

    let mut current_msg = String::new();
    if history.last().is_some_and(|(r, _)| r == "user") {
        current_msg = history.pop().map(|(_, c)| c).unwrap_or_default();
    }

    ParsedMessages {
        system_msg,
        history,
        current_msg,
    }
}

fn message_content(msg: &Value) -> String {
    match msg.get("content") {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Array(parts)) => parts
            .iter()
            .filter_map(|p| {
                if p.get("type").and_then(Value::as_str) == Some("text") {
                    p.get("text").and_then(Value::as_str).map(str::to_string)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
            .join(" "),
        _ => String::new(),
    }
}

pub fn build_conversation_body(
    parsed: &ParsedMessages,
    model_slug: &str,
    parent_message_id: &str,
) -> Value {
    let mut system_parts = Vec::new();
    if !parsed.system_msg.trim().is_empty() {
        system_parts.push(parsed.system_msg.trim().to_string());
    }
    if !parsed.history.is_empty() {
        let formatted = parsed
            .history
            .iter()
            .map(|(role, content)| {
                if role == "assistant" {
                    format!("Assistant: {content}")
                } else {
                    format!("User: {content}")
                }
            })
            .collect::<Vec<_>>()
            .join("\n\n");
        system_parts.push(format!(
            "Prior conversation (for context — answer only the new user \
             message below):\n\n{formatted}"
        ));
    }

    let mut messages = Vec::new();
    if !system_parts.is_empty() {
        messages.push(serde_json::json!({
            "id": uuid::Uuid::new_v4().to_string(),
            "author": { "role": "system" },
            "content": { "content_type": "text", "parts": [system_parts.join("\n\n")] },
        }));
    }
    messages.push(serde_json::json!({
        "id": uuid::Uuid::new_v4().to_string(),
        "author": { "role": "user" },
        "content": { "content_type": "text", "parts": [parsed.current_msg] },
    }));

    serde_json::json!({
        "action": "next",
        "messages": messages,
        "model": model_slug,
        "conversation_id": null,
        "parent_message_id": parent_message_id,
        "timezone_offset_min": chrono::Local::now().offset().local_minus_utc() / 60,
        "history_and_training_disabled": true,
        "suggestions": [],
        "websocket_request_id": uuid::Uuid::new_v4().to_string(),
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn folds_history_into_system() {
        let msgs = vec![
            json!({"role":"user","content":"hi"}),
            json!({"role":"assistant","content":"hello"}),
            json!({"role":"user","content":"again"}),
        ];
        let parsed = parse_openai_messages(&msgs);
        assert_eq!(parsed.current_msg, "again");
        assert_eq!(parsed.history.len(), 2);
        let body = build_conversation_body(&parsed, "gpt-5-mini", "parent");
        let msgs = body.get("messages").unwrap().as_array().unwrap();
        assert_eq!(msgs.len(), 2);
    }

    #[test]
    fn system_schema_instruction_reaches_chatgpt_body() {
        let schema_hint = "Output ONLY the JSON object in the message content";
        let msgs = vec![
            json!({
                "role": "system",
                "content": format!("You must respond with valid JSON.\n{schema_hint}")
            }),
            json!({"role":"user","content":"extract entity"}),
        ];
        let parsed = parse_openai_messages(&msgs);
        let body = build_conversation_body(&parsed, "gpt-5-mini", "parent");
        let system = &body["messages"][0]["content"]["parts"][0];
        let text = system.as_str().unwrap();
        assert!(text.contains(schema_hint));
        assert!(text.contains("valid JSON"));
    }

    #[test]
    fn json_retry_suffix_appends_to_system() {
        let mut parsed = parse_openai_messages(&[
            json!({
                "role": "system",
                "content": "schema rules"
            }),
            json!({"role":"user","content":"go"}),
        ]);
        parsed
            .system_msg
            .push_str(crate::constants::JSON_RETRY_SUFFIX);
        let body = build_conversation_body(&parsed, "gpt-5-mini", "parent");
        let text = body["messages"][0]["content"]["parts"][0].as_str().unwrap();
        assert!(text.contains("CRITICAL"));
        assert!(text.contains("ONLY a JSON object"));
    }
}
