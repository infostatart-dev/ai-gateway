use serde_json::Value;
use web_message_budget::WebTurn;

pub fn web_turn_to_prompt(turn: &WebTurn) -> String {
    let mut parts = Vec::new();
    if !turn.system_msg.trim().is_empty() {
        parts.push(turn.system_msg.trim().to_string());
    }
    if !turn.user_msg.trim().is_empty() {
        parts.push(turn.user_msg.trim().to_string());
    }
    strip_images(&parts.join("\n\n"))
}

pub fn messages_to_prompt(messages: &[Value], history_window: usize) -> String {
    if messages.is_empty() {
        return String::new();
    }

    let mut system_parts = Vec::new();
    let mut conversation = Vec::new();
    let mut last_user = String::new();

    for msg in messages {
        let role = msg.get("role").and_then(Value::as_str).unwrap_or("");
        let text = extract_message_text(msg.get("content")).trim().to_string();
        match role {
            "system" if !text.is_empty() => system_parts.push(text),
            "user" | "assistant" if !text.is_empty() => {
                conversation.push((role.to_string(), text.clone()));
                if role == "user" {
                    last_user = text;
                }
            }
            _ => {}
        }
    }

    let mut parts = Vec::new();
    if !system_parts.is_empty() {
        parts.push(system_parts.join("\n\n"));
    }

    if history_window > 0 && conversation.len() > 1 {
        let transcript = conversation
            .iter()
            .rev()
            .take(history_window)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .map(|(role, text)| {
                let label = if role == "assistant" {
                    "Assistant"
                } else {
                    "User"
                };
                format!("{label}: {text}")
            })
            .collect::<Vec<_>>()
            .join("\n\n");
        parts.push(transcript);
    } else if !last_user.is_empty() {
        parts.push(last_user);
    }

    strip_images(&parts.join("\n\n"))
}

fn extract_message_text(content: Option<&Value>) -> String {
    match content {
        Some(Value::Array(items)) => items
            .iter()
            .filter(|i| i.get("type").and_then(Value::as_str) == Some("text"))
            .filter_map(|i| i.get("text").and_then(Value::as_str))
            .collect::<Vec<_>>()
            .join("\n"),
        Some(Value::String(s)) => s.clone(),
        Some(other) => other.as_str().unwrap_or("").to_string(),
        None => String::new(),
    }
}

fn strip_images(text: &str) -> String {
    regex::Regex::new(r"!\[.*?\]\(.*?\)")
        .ok()
        .map(|re| re.replace_all(text, "").to_string())
        .unwrap_or_else(|| text.to_string())
}

#[cfg(test)]
mod tests {
    use web_message_budget::{WebTurn, WebTurnKind};

    use super::*;

    #[test]
    fn combines_system_and_user_in_order() {
        let turn = WebTurn {
            kind: WebTurnKind::Final,
            system_msg: "Rules".into(),
            user_msg: "Question".into(),
        };
        let prompt = web_turn_to_prompt(&turn);
        assert!(prompt.starts_with("Rules"));
        assert!(prompt.contains("Question"));
    }
}
