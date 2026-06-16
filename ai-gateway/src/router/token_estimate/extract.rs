use serde_json::Value;

/// Build the billable text for token estimation: all message content plus the
/// serialized tool definitions and `response_format` `json_schema` (the fat
/// schema is a large share of Sales-QA payloads). Over-counting structural
/// punctuation is acceptable and conservative for filtering.
#[must_use]
pub fn billable_text(body: &Value) -> String {
    let mut out = String::new();
    if let Some(messages) = body.get("messages").and_then(Value::as_array) {
        for message in messages {
            push_message(message, &mut out);
        }
    }
    for key in ["tools", "tool_choice", "response_format", "system"] {
        if let Some(value) = body.get(key) {
            out.push('\n');
            push_value_text(value, &mut out);
        }
    }
    out
}

fn push_message(message: &Value, out: &mut String) {
    if let Some(role) = message.get("role").and_then(Value::as_str) {
        out.push_str(role);
        out.push('\n');
    }
    if let Some(content) = message.get("content") {
        push_value_text(content, out);
        out.push('\n');
    }
    if let Some(calls) = message.get("tool_calls") {
        push_value_text(calls, out);
        out.push('\n');
    }
}

/// Collect human-readable text from a JSON value: plain strings verbatim,
/// `{type:text,text:...}` parts by their text, everything else by its compact
/// JSON serialization (a safe upper bound).
fn push_value_text(value: &Value, out: &mut String) {
    match value {
        Value::String(text) => out.push_str(text),
        Value::Array(items) => {
            for item in items {
                push_value_text(item, out);
                out.push('\n');
            }
        }
        Value::Object(map) => match map.get("text").and_then(Value::as_str) {
            Some(text) => out.push_str(text),
            None => out.push_str(&value.to_string()),
        },
        other => out.push_str(&other.to_string()),
    }
}
