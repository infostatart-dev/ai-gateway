use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelOptions {
    pub model_type: String,
    pub thinking_enabled: bool,
    pub search_enabled: bool,
}

pub fn resolve_model_options(model: &str, body: &Value) -> ModelOptions {
    let m = model.to_ascii_lowercase();
    let model_type = if m.contains("pro") || m.contains("expert") {
        "expert"
    } else {
        "default"
    }
    .to_string();

    let thinking_enabled = m.contains("r1")
        || m.contains("think")
        || m.contains("reason")
        || body.get("thinking_enabled").and_then(Value::as_bool) == Some(true)
        || body.get("thinking").and_then(Value::as_bool) == Some(true)
        || body.get("reasoning_effort").is_some();

    let search_enabled = m.contains("search")
        || body.get("search_enabled").and_then(Value::as_bool) == Some(true)
        || body.get("search").and_then(Value::as_bool) == Some(true)
        || body.get("web_search").and_then(Value::as_bool) == Some(true);

    ModelOptions {
        model_type,
        thinking_enabled,
        search_enabled,
    }
}
