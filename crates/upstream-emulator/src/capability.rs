use ai_gateway::{
    config::{model_capability, providers::ProvidersConfig},
    types::provider::InferenceProvider,
};
use serde_json::Value;
use web_structured_output::{
    parse_json_schema_spec, request_requires_json_object,
};

use crate::{schema_fill::fill_minimal_valid_json, welcome::WELCOME};

pub enum ContentResult {
    /// Provider supports the requested format; return this string as content.
    Ok(String),
    /// Request requires `json_schema` but provider does not support it.
    UnsupportedFormat,
}

#[must_use]
pub fn resolve_content(
    body: &Value,
    providers: &ProvidersConfig,
    provider: &InferenceProvider,
) -> ContentResult {
    let model = body
        .get("model")
        .and_then(Value::as_str)
        .unwrap_or("default");
    let wants_json_schema = parse_json_schema_spec(body).is_some();
    let supports_schema =
        model_capability::supports_json_schema(providers, provider, model);

    if wants_json_schema && !supports_schema {
        return ContentResult::UnsupportedFormat;
    }
    if wants_json_schema {
        if let Some(spec) = parse_json_schema_spec(body) {
            let value = fill_minimal_valid_json(&spec.schema);
            return ContentResult::Ok(
                serde_json::to_string(&value).unwrap_or_else(|_| "{}".into()),
            );
        }
    }
    if request_requires_json_object(body) {
        return ContentResult::Ok(r#"{"ok":true}"#.into());
    }
    ContentResult::Ok(WELCOME.into())
}

/// Convenience wrapper — returns plain content string without capability check.
/// Use only when capability is already verified upstream.
#[must_use]
pub fn assistant_content(
    body: &Value,
    providers: &ProvidersConfig,
    provider: &InferenceProvider,
) -> String {
    match resolve_content(body, providers, provider) {
        ContentResult::Ok(s) => s,
        ContentResult::UnsupportedFormat => WELCOME.into(),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn yaml_capable_model_emits_schema_json() {
        let providers = ProvidersConfig::default();
        let provider = InferenceProvider::Named("longcat".into());
        let body = json!({
            "model": "LongCat-2.0-Preview",
            "response_format": {
                "type": "json_schema",
                "json_schema": {
                    "strict": true,
                    "schema": {
                        "type": "object",
                        "properties": { "status": { "type": "string" } },
                        "required": ["status"],
                        "additionalProperties": false
                    }
                }
            }
        });
        let content = assistant_content(&body, &providers, &provider);
        assert_eq!(content, r#"{"status":"ok"}"#);
    }

    #[test]
    fn yaml_non_capable_model_stays_plain_ok() {
        let providers = ProvidersConfig::default();
        let provider = InferenceProvider::Named("ollama-cloud".into());
        let body = json!({
            "model": "deepseek-v4-pro",
            "response_format": {
                "type": "json_schema",
                "json_schema": {
                    "strict": true,
                    "schema": {
                        "type": "object",
                        "properties": { "status": { "type": "string" } },
                        "required": ["status"],
                        "additionalProperties": false
                    }
                }
            }
        });
        assert_eq!(
            assistant_content(&body, &providers, &provider),
            super::super::welcome::WELCOME
        );
    }
}
