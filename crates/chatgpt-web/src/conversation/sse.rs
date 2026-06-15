use serde_json::Value;

#[derive(Debug, Clone)]
pub struct SseEvent {
    pub message: Option<MessageEvent>,
    pub conversation_id: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct MessageEvent {
    pub id: Option<String>,
    pub role: Option<String>,
    pub parts: Vec<String>,
    pub status: Option<String>,
}

pub fn parse_sse_events(raw: &str) -> Vec<SseEvent> {
    let mut events = Vec::new();
    let mut data_lines = Vec::new();

    for line in raw.lines() {
        let line = line.strip_suffix('\r').unwrap_or(line);
        if line.is_empty() {
            if let Some(event) = flush_data(&mut data_lines) {
                events.push(event);
            }
            continue;
        }
        if let Some(payload) = line.strip_prefix("data:") {
            data_lines.push(payload.trim_start().to_string());
        }
    }
    if let Some(event) = flush_data(&mut data_lines) {
        events.push(event);
    }
    events
}

fn flush_data(lines: &mut Vec<String>) -> Option<SseEvent> {
    if lines.is_empty() {
        return None;
    }
    let payload = lines.join("\n");
    lines.clear();
    let trimmed = payload.trim();
    if trimmed.is_empty() || trimmed == "[DONE]" {
        return None;
    }
    let value: Value = serde_json::from_str(trimmed).ok()?;
    if let Some(err) = value.get("error").filter(|e| !e.is_null()) {
        let msg = err
            .as_str()
            .or_else(|| err.get("message").and_then(Value::as_str))
            .map(str::to_string)
            .unwrap_or_else(|| err.to_string());
        return Some(SseEvent {
            message: None,
            conversation_id: None,
            error: Some(msg),
        });
    }
    let message = value.get("message").map(|m| MessageEvent {
        id: m.get("id").and_then(Value::as_str).map(str::to_string),
        role: m
            .pointer("/author/role")
            .and_then(Value::as_str)
            .map(str::to_string),
        parts: m
            .pointer("/content/parts")
            .and_then(Value::as_array)
            .map(|arr| {
                arr.iter()
                    .filter_map(|p| p.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default(),
        status: m.get("status").and_then(Value::as_str).map(str::to_string),
    });
    Some(SseEvent {
        message,
        conversation_id: value
            .get("conversation_id")
            .and_then(Value::as_str)
            .map(str::to_string),
        error: None,
    })
}

pub fn collect_sse_content(raw: &str) -> Result<String, String> {
    let mut current_id: Option<String> = None;
    let mut current_parts = String::new();
    let mut is_live = false;

    for event in parse_sse_events(raw) {
        if let Some(err) = event.error {
            return Err(err);
        }
        let Some(msg) = event.message else { continue };
        if msg.role.as_deref() != Some("assistant") {
            continue;
        }
        if msg.id.as_ref() != current_id.as_ref() {
            current_id = msg.id.clone();
            current_parts.clear();
            is_live = false;
        }
        if msg.status.as_deref() == Some("in_progress") {
            is_live = true;
        }
        let cumulative = msg.parts.join("");
        if cumulative.len() > current_parts.len() {
            current_parts = cumulative;
        }
    }

    if current_parts.is_empty() && !is_live {
        return Ok(String::new());
    }
    Ok(clean_text(&current_parts))
}

fn clean_text(text: &str) -> String {
    regex::Regex::new(r"entity\[[^\]]+\]")
        .ok()
        .map(|re| re.replace_all(text, "").to_string())
        .unwrap_or_else(|| text.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cumulative_content() {
        let raw = "\
data: {\"message\":{\"id\":\"m1\",\"author\":{\"role\":\"assistant\"},\"\
                   content\":{\"parts\":[\"Hel\"]},\"status\":\"in_progress\"\
                   }}\n\ndata: \
                   {\"message\":{\"id\":\"m1\",\"author\":{\"role\":\"\
                   assistant\"},\"content\":{\"parts\":[\"Hello\"]},\"status\"\
                   :\"in_progress\"}}\n\ndata: \
                   {\"message\":{\"id\":\"m1\",\"author\":{\"role\":\"\
                   assistant\"},\"content\":{\"parts\":[\"Hello\"]},\"status\"\
                   :\"finished_successfully\"}}\n\ndata: [DONE]\n";
        assert_eq!(collect_sse_content(raw).unwrap(), "Hello");
    }

    #[test]
    fn ignores_null_error_events() {
        let raw = "\
data: {\"error\":null}\n\ndata: \
                   {\"message\":{\"id\":\"m1\",\"author\":{\"role\":\"\
                   assistant\"},\"content\":{\"parts\":[\"Hi\"]},\"status\":\"\
                   finished_successfully\"}}\n\ndata: [DONE]\n";
        assert_eq!(collect_sse_content(raw).unwrap(), "Hi");
    }
}
