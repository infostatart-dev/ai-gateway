//! Shared structured-output parsing, prompt injection, and validation for
//! browser-session LLM providers.

mod instruct;
mod parse;
mod retry;
mod validate;

pub use instruct::{
    base_system_without_schema, build_json_object_instruction,
    build_schema_instruction,
};
pub use parse::{
    JsonSchemaSpec, StructuredOutputIssue, StructuredOutputMode,
    parse_json_schema_spec, request_requires_json_object,
    request_requires_json_schema, structured_output_mode,
};
pub use retry::{JSON_RETRY_SUFFIX, SCHEMA_RETRY_SUFFIX, retry_suffix_for};
pub use validate::{check_structured_response, content_matches_schema};

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn assistant(content: &str) -> serde_json::Value {
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

    #[test]
    fn json_object_mode_parses_json_only() {
        let body = json!({ "response_format": { "type": "json_object" } });
        assert_eq!(
            structured_output_mode(&body),
            Some(StructuredOutputMode::JsonObject)
        );
        assert!(
            check_structured_response(&assistant(r#"{"ok":true}"#), None,)
                .is_none()
        );
    }
}
