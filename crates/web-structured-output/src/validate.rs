use serde_json::Value;

use super::parse::{JsonSchemaSpec, StructuredOutputIssue};

/// Returns `None` when structured output is valid (or not required).
#[must_use]
pub fn check_structured_response(
    response: &Value,
    spec: Option<&JsonSchemaSpec>,
) -> Option<StructuredOutputIssue> {
    let Some(content) = response
        .pointer("/choices/0/message/content")
        .and_then(Value::as_str)
    else {
        return Some(StructuredOutputIssue::InvalidJson);
    };
    if content.trim().is_empty() {
        return Some(StructuredOutputIssue::InvalidJson);
    }

    let parsed = match serde_json::from_str::<Value>(content.trim()) {
        Ok(value) => value,
        Err(_) => return Some(StructuredOutputIssue::InvalidJson),
    };

    let spec = spec?;

    if content_matches_schema(&parsed, &spec.schema) {
        None
    } else {
        Some(StructuredOutputIssue::SchemaMismatch)
    }
}

#[must_use]
pub fn content_matches_schema(instance: &Value, schema: &Value) -> bool {
    jsonschema::validator_for(schema)
        .ok()
        .is_some_and(|validator| validator.is_valid(instance))
}
