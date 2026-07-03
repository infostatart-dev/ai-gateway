use serde_json::Value;

use crate::error::mapper::MapperError;

pub fn normalize_chat_completion(value: &mut Value) {
    ensure_openai_envelope(value, "chat.completion");
    flatten_choice_message_content(value);
}

pub fn normalize_stream_chunk(value: &mut Value) {
    ensure_openai_envelope(value, "chat.completion.chunk");
    flatten_choice_message_content(value);
}

pub fn ensure_non_empty_choices(value: &Value) -> Result<(), MapperError> {
    let choices = value.get("choices").ok_or_else(|| {
        MapperError::UnsupportedFormat("missing choices".into())
    })?;
    let Some(items) = choices.as_array() else {
        return Err(MapperError::UnsupportedFormat(
            "choices must be an array".into(),
        ));
    };
    if items.is_empty() {
        return Err(MapperError::UnsupportedFormat("empty choices".into()));
    }
    Ok(())
}

fn ensure_openai_envelope(value: &mut Value, object: &str) {
    let Some(obj) = value.as_object_mut() else {
        return;
    };
    if !obj.contains_key("id") {
        obj.insert(
            "id".to_string(),
            Value::String(format!(
                "chatcmpl-{}",
                uuid::Uuid::new_v4().simple()
            )),
        );
    }
    if !obj.contains_key("object") {
        obj.insert("object".to_string(), Value::String(object.to_string()));
    }
    if !obj.contains_key("created") {
        let created = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |duration| duration.as_secs());
        obj.insert("created".to_string(), Value::Number(created.into()));
    }
}

fn flatten_choice_message_content(value: &mut Value) {
    let Some(choices) = value.get_mut("choices").and_then(Value::as_array_mut)
    else {
        return;
    };
    for choice in choices {
        if let Some(message) = choice.get_mut("message") {
            flatten_message_content_field(message);
        }
        if let Some(delta) = choice.get_mut("delta") {
            flatten_message_content_field(delta);
        }
    }
}

fn flatten_message_content_field(message: &mut Value) {
    let Some(obj) = message.as_object_mut() else {
        return;
    };
    let content = obj.get("content").cloned();
    let Some(content) = content else {
        return;
    };
    if content.is_string() {
        return;
    }
    if let Some(text) = json_content_to_string(&content) {
        obj.insert("content".to_string(), Value::String(text));
    }
}

pub fn json_content_to_string(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => Some(text.clone()),
        Value::Array(parts) => {
            let text: String =
                parts.iter().filter_map(content_part_to_text).collect();
            (!text.is_empty()).then_some(text)
        }
        Value::Object(map) => map
            .get("text")
            .and_then(Value::as_str)
            .map(str::to_string)
            .or_else(|| {
                map.get("content")
                    .and_then(Value::as_str)
                    .map(str::to_string)
            }),
        _ => None,
    }
}

fn content_part_to_text(part: &Value) -> Option<String> {
    part.get("text")
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| part.as_str().map(str::to_string))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flattens_object_content_to_string() {
        let mut value = serde_json::json!({
            "choices": [{
                "message": {
                    "role": "assistant",
                    "content": {"type": "text", "text": "hello"}
                }
            }]
        });
        normalize_chat_completion(&mut value);
        assert_eq!(value["choices"][0]["message"]["content"], "hello");
    }

    #[test]
    fn flattens_array_content_to_string() {
        let mut value = serde_json::json!({
            "choices": [{
                "message": {
                    "role": "assistant",
                    "content": [{"type": "text", "text": "part-a"}, {"type": "text", "text": "part-b"}]
                }
            }]
        });
        normalize_chat_completion(&mut value);
        assert_eq!(value["choices"][0]["message"]["content"], "part-apart-b");
    }

    #[test]
    fn rejects_missing_choices() {
        let value = serde_json::json!({"model": "x", "usage": {}});
        assert!(ensure_non_empty_choices(&value).is_err());
    }
}
