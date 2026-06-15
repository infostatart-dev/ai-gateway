use serde_json::Value;

const ONLY_JSON_TAIL: &str = "Output ONLY the JSON object in the message \
                              content. No markdown fences, no prose.";

const STRICT_MANDATORY: &str = "MANDATORY strict mode: you MUST NOT omit any required field, use wrong \
     types, or add extra properties. The response must be a single JSON \
     object that validates against the schema below — no exceptions.";

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

#[must_use]
pub fn request_requires_json_schema(body: &Value) -> bool {
    body.get("response_format")
        .and_then(|rf| rf.get("type"))
        .and_then(Value::as_str)
        == Some("json_schema")
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

#[must_use]
pub fn build_schema_instruction(spec: &JsonSchemaSpec) -> String {
    let mut lines = Vec::new();
    if spec.strict {
        lines.push(STRICT_MANDATORY.into());
    }
    let schema = serde_json::to_string_pretty(&spec.schema)
        .unwrap_or_else(|_| "{}".into());
    lines.push(format!(
        "You must respond with valid JSON that strictly follows this JSON schema:\n{schema}"
    ));
    lines.push(ONLY_JSON_TAIL.into());
    lines.join("\n")
}

/// System text without the schema block (for multi-turn context uploads).
#[must_use]
pub fn base_system_without_schema(
    system_msg: &str,
    schema_instruction: Option<&str>,
) -> String {
    let Some(schema) = schema_instruction else {
        return system_msg.trim().to_string();
    };
    system_msg
        .replace(schema, "")
        .trim()
        .trim_end_matches('\n')
        .to_string()
}

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

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn assistant(content: &str) -> Value {
        json!({ "choices": [{ "message": { "content": content } }] })
    }

    fn status_schema_spec() -> JsonSchemaSpec {
        JsonSchemaSpec {
            strict: true,
            schema: json!({
                "type": "object",
                "properties": { "status": { "type": "string" } },
                "required": ["status"],
                "additionalProperties": false
            }),
        }
    }

    #[test]
    fn parses_strict_schema_from_request() {
        let body = json!({
            "response_format": {
                "type": "json_schema",
                "json_schema": {
                    "name": "out",
                    "strict": true,
                    "schema": { "type": "object" }
                }
            }
        });
        let spec = parse_json_schema_spec(&body).unwrap();
        assert!(spec.strict);
        assert_eq!(spec.schema, json!({ "type": "object" }));
    }

    #[test]
    fn accepts_json_matching_schema() {
        let spec = status_schema_spec();
        assert!(
            check_structured_response(
                &assistant(r#"{"status":"ok"}"#),
                Some(&spec),
            )
            .is_none()
        );
    }

    #[test]
    fn rejects_valid_json_that_violates_schema() {
        let spec = status_schema_spec();
        assert_eq!(
            check_structured_response(
                &assistant(r#"{"status":42}"#),
                Some(&spec),
            ),
            Some(StructuredOutputIssue::SchemaMismatch)
        );
    }

    #[test]
    fn rejects_prose_instead_of_json() {
        let spec = status_schema_spec();
        assert_eq!(
            check_structured_response(
                &assistant("Here is your JSON: {\"status\":\"ok\"}"),
                Some(&spec),
            ),
            Some(StructuredOutputIssue::InvalidJson)
        );
    }
}
