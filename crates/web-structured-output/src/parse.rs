use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StructuredOutputIssue {
    InvalidJson,
    SchemaMismatch,
}

#[derive(Debug, Clone)]
pub struct JsonSchemaSpec {
    pub schema: Value,
    pub strict: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StructuredOutputMode {
    JsonSchema,
    JsonObject,
}

#[must_use]
pub fn request_requires_json_schema(body: &Value) -> bool {
    body.get("response_format")
        .and_then(|rf| rf.get("type"))
        .and_then(Value::as_str)
        == Some("json_schema")
}

#[must_use]
pub fn request_requires_json_object(body: &Value) -> bool {
    body.get("response_format")
        .and_then(|rf| rf.get("type"))
        .and_then(Value::as_str)
        == Some("json_object")
}

#[must_use]
pub fn structured_output_mode(body: &Value) -> Option<StructuredOutputMode> {
    if request_requires_json_schema(body) {
        Some(StructuredOutputMode::JsonSchema)
    } else if request_requires_json_object(body) {
        Some(StructuredOutputMode::JsonObject)
    } else {
        None
    }
}

#[must_use]
pub fn parse_json_schema_spec(body: &Value) -> Option<JsonSchemaSpec> {
    if !request_requires_json_schema(body) {
        return None;
    }
    let json_schema = body.get("response_format")?.get("json_schema")?;
    Some(JsonSchemaSpec {
        schema: json_schema.get("schema")?.clone(),
        strict: json_schema
            .get("strict")
            .and_then(Value::as_bool)
            .unwrap_or(false),
    })
}
