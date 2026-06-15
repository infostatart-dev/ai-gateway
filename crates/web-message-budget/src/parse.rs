use serde_json::Value;

use crate::types::ParsedChat;

pub fn parse_openai_messages(messages: &[Value]) -> ParsedChat {
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

    ParsedChat {
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

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn splits_last_user_as_current() {
        let parsed = parse_openai_messages(&[
            json!({"role":"user","content":"hi"}),
            json!({"role":"assistant","content":"hello"}),
            json!({"role":"user","content":"again"}),
        ]);
        assert_eq!(parsed.current_msg, "again");
        assert_eq!(parsed.history.len(), 2);
    }
}
