use bytes::Bytes;
use chatgpt_web::schema::{check_structured_response, parse_json_schema_spec};
use serde_json::Value;

use crate::router::capability::{ModelCapability, RequestRequirements};

/// Returns false when the client asked for JSON schema output, the candidate
/// advertises JSON schema support, the request is non-streaming, and the
/// assistant content is missing, not valid JSON, or fails schema validation.
pub(super) fn structured_output_valid(
    requirements: &RequestRequirements,
    capability: &ModelCapability,
    request_body: &Bytes,
    response_body: &Bytes,
) -> bool {
    if !requirements.json_schema_required || !capability.supports_json_schema {
        return true;
    }
    if request_is_stream(request_body) {
        return true;
    }

    let Ok(request) = serde_json::from_slice::<Value>(request_body) else {
        return false;
    };
    let Ok(response) = serde_json::from_slice::<Value>(response_body) else {
        return false;
    };

    check_structured_response(&response, parse_json_schema_spec(&request).as_ref()).is_none()
}

pub(super) fn request_is_stream(request_body: &Bytes) -> bool {
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(request_body)
    else {
        return false;
    };
    value
        .get("stream")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false)
}

pub(super) fn chat_content_is_valid_json(response_body: &Bytes) -> bool {
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(response_body)
    else {
        return false;
    };

    let Some(content) = value
        .pointer("/choices/0/message/content")
        .and_then(serde_json::Value::as_str)
    else {
        return false;
    };

    if content.trim().is_empty() {
        return false;
    }

    serde_json::from_str::<serde_json::Value>(content).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{model_id::ModelId, provider::InferenceProvider};
    use serde_json::json;

    fn json_schema_request(stream: bool, strict: bool) -> Bytes {
        Bytes::from(format!(
            r#"{{"model":"openai/gpt-5-mini","stream":{stream},"response_format":{{"type":"json_schema","json_schema":{{"name":"out","strict":{strict},"schema":{{"type":"object","properties":{{"status":{{"type":"string"}}}},"required":["status"],"additionalProperties":false}}}}}},"messages":[{{"role":"user","content":"hi"}}]}}"#
        ))
    }

    fn capability(supports_json_schema: bool) -> ModelCapability {
        ModelCapability {
            provider: InferenceProvider::OpenRouter,
            model: ModelId::from_str_and_provider(
                InferenceProvider::OpenRouter,
                "openai/gpt-oss-120b:free",
            )
            .unwrap(),
            context_window: Some(131_072),
            supports_tools: true,
            supports_json_schema,
            supports_vision: false,
            reasoning: false,
        }
    }

    #[test]
    fn accepts_valid_json_content() {
        let requirements = RequestRequirements {
            json_schema_required: true,
            ..RequestRequirements::default()
        };
        let response = Bytes::from(
            r#"{"choices":[{"message":{"content":"{\"status\":\"ok\"}"}}]}"#,
        );
        assert!(structured_output_valid(
            &requirements,
            &capability(true),
            &json_schema_request(false, true),
            &response,
        ));
    }

    #[test]
    fn rejects_json_that_fails_schema_validation() {
        let requirements = RequestRequirements {
            json_schema_required: true,
            ..RequestRequirements::default()
        };
        let response = Bytes::from(
            r#"{"choices":[{"message":{"content":"{\"status\":42}"}}]}"#,
        );
        assert!(!structured_output_valid(
            &requirements,
            &capability(true),
            &json_schema_request(false, true),
            &response,
        ));
    }

    #[test]
    fn rejects_markdown_instead_of_json() {
        let requirements = RequestRequirements {
            json_schema_required: true,
            ..RequestRequirements::default()
        };
        let response = Bytes::from(
            r#"{"choices":[{"message":{"content":"| col |\\n| --- |"}}]}"#,
        );
        assert!(!structured_output_valid(
            &requirements,
            &capability(true),
            &json_schema_request(false, true),
            &response,
        ));
    }

    #[test]
    fn rejects_truncated_json_content() {
        let requirements = RequestRequirements {
            json_schema_required: true,
            ..RequestRequirements::default()
        };
        let response = Bytes::from(
            r#"{"choices":[{"message":{"content":"{\"ok\":true"}}]}"#,
        );
        assert!(!structured_output_valid(
            &requirements,
            &capability(true),
            &json_schema_request(false, true),
            &response,
        ));
    }

    #[test]
    fn rejects_empty_content() {
        let requirements = RequestRequirements {
            json_schema_required: true,
            ..RequestRequirements::default()
        };
        let response = Bytes::from(r#"{"choices":[{"message":{"content":""}}]}"#);
        assert!(!structured_output_valid(
            &requirements,
            &capability(true),
            &json_schema_request(false, true),
            &response,
        ));
    }

    #[test]
    fn skips_check_for_streaming_requests() {
        let requirements = RequestRequirements {
            json_schema_required: true,
            ..RequestRequirements::default()
        };
        let response = Bytes::from(r#"{"choices":[{"message":{"content":"not json"}}]}"#);
        assert!(structured_output_valid(
            &requirements,
            &capability(true),
            &json_schema_request(true, true),
            &response,
        ));
    }

    #[test]
    fn skips_check_when_candidate_does_not_advertise_json_schema() {
        let requirements = RequestRequirements {
            json_schema_required: true,
            ..RequestRequirements::default()
        };
        let response = Bytes::from(r#"{"choices":[{"message":{"content":"nope"}}]}"#);
        assert!(structured_output_valid(
            &requirements,
            &capability(false),
            &json_schema_request(false, true),
            &response,
        ));
    }

    #[test]
    fn parses_strict_schema_from_request_body() {
        let request = json_schema_request(false, true);
        let parsed = parse_json_schema_spec(
            &serde_json::from_slice::<Value>(&request).unwrap(),
        )
        .unwrap();
        assert!(parsed.strict);
    }
}
