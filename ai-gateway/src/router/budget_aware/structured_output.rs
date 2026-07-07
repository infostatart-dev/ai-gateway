use bytes::Bytes;
use serde_json::{Value, json};
use web_structured_output::{
    StructuredOutputIssue, check_structured_response, parse_json_schema_spec,
    retry_suffix_for,
};

use crate::{
    router::capability::{ModelCapability, RequestRequirements},
    types::provider::InferenceProvider,
};

const REFLECTOR_SYSTEM_PROMPT: &str =
    "You are a JSON Schema conformance reflector. You were asked to return \
     JSON that validates against the supplied schema, but the previous \
     assistant response did not validate. Return one JSON object that exactly \
     matches the schema. Do not wrap it in markdown or code fences. Preserve \
     the original facts, meaning, language, and user-facing text; do not \
     rewrite, summarize, translate, or replace text except where the schema \
     forces field placement.";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum StructuredOutputValidation {
    Valid,
    Skipped,
    Invalid(StructuredOutputIssue),
}

impl StructuredOutputValidation {
    pub(super) fn is_valid_or_skipped(self) -> bool {
        matches!(self, Self::Valid | Self::Skipped)
    }

    pub(super) fn issue(self) -> Option<StructuredOutputIssue> {
        match self {
            Self::Invalid(issue) => Some(issue),
            Self::Valid | Self::Skipped => None,
        }
    }
}

/// Returns false when the client asked for JSON schema output, the candidate
/// advertises JSON schema support, the request is non-streaming, and the
/// assistant content is missing, not valid JSON, or fails schema validation.
#[cfg(test)]
fn structured_output_valid(
    requirements: &RequestRequirements,
    capability: &ModelCapability,
    request_body: &Bytes,
    response_body: &Bytes,
) -> bool {
    validate_structured_output(
        requirements,
        capability,
        request_body,
        response_body,
    )
    .is_valid_or_skipped()
}

pub(super) fn validate_structured_output(
    requirements: &RequestRequirements,
    capability: &ModelCapability,
    request_body: &Bytes,
    response_body: &Bytes,
) -> StructuredOutputValidation {
    if !requirements.json_schema_required || !capability.supports_json_schema {
        return StructuredOutputValidation::Skipped;
    }
    if request_is_stream(request_body) {
        return StructuredOutputValidation::Skipped;
    }

    let Ok(request) = serde_json::from_slice::<Value>(request_body) else {
        return StructuredOutputValidation::Invalid(
            StructuredOutputIssue::InvalidJson,
        );
    };
    let Ok(response) = serde_json::from_slice::<Value>(response_body) else {
        return StructuredOutputValidation::Invalid(
            StructuredOutputIssue::InvalidJson,
        );
    };

    match check_structured_response(
        &response,
        parse_json_schema_spec(&request).as_ref(),
    ) {
        Some(issue) => StructuredOutputValidation::Invalid(issue),
        None => StructuredOutputValidation::Valid,
    }
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

pub(super) fn schema_conformance_reflector_enabled(
    provider: &InferenceProvider,
) -> bool {
    matches!(provider, InferenceProvider::Named(name) if name == "llm7" || name == "longcat")
}

pub(super) fn markdown_fenced_json_normalizer_enabled(
    provider: &InferenceProvider,
) -> bool {
    matches!(provider, InferenceProvider::Named(name) if name == "llm7")
}

pub(super) fn normalize_markdown_fenced_json_response(
    response_body: &Bytes,
) -> Option<Bytes> {
    let mut response = serde_json::from_slice::<Value>(response_body).ok()?;
    let content = response
        .pointer("/choices/0/message/content")
        .and_then(Value::as_str)?;
    let normalized = extract_single_markdown_fenced_json(content)?.to_string();

    response
        .pointer_mut("/choices/0/message/content")?
        .as_str()?;
    response["choices"][0]["message"]["content"] = Value::String(normalized);

    serde_json::to_vec(&response).ok().map(Bytes::from)
}

fn extract_single_markdown_fenced_json(content: &str) -> Option<&str> {
    let trimmed = content.trim();
    let rest = trimmed.strip_prefix("```")?;
    let (language, after_language) = rest.split_once('\n')?;
    let language = language.trim();
    if !language.is_empty() && !language.eq_ignore_ascii_case("json") {
        return None;
    }
    let inner = after_language.strip_suffix("```")?.trim();
    if inner.is_empty() {
        return None;
    }
    serde_json::from_str::<Value>(inner).ok()?;
    Some(inner)
}

pub(super) fn build_schema_conformance_reflection_request(
    request_body: &Bytes,
    response_body: &Bytes,
) -> Option<Bytes> {
    let mut request = serde_json::from_slice::<Value>(request_body).ok()?;
    let schema = request
        .pointer("/response_format/json_schema/schema")
        .cloned()
        .or_else(|| request.get("response_format").cloned())?;
    let invalid_text = assistant_content(response_body)
        .unwrap_or_else(|| String::from_utf8_lossy(response_body).into_owned());

    let user_prompt = format!(
        "Original JSON Schema:\n{}\n\nInvalid assistant \
         response:\n{}\n\nReturn exactly one JSON object that validates \
         against the schema. Keep the original text and meaning; only move it \
         into the required JSON fields.",
        serde_json::to_string_pretty(&schema).ok()?,
        invalid_text
    );

    request["messages"] = json!([
        {
            "role": "system",
            "content": REFLECTOR_SYSTEM_PROMPT
        },
        {
            "role": "user",
            "content": user_prompt
        }
    ]);
    request["stream"] = Value::Bool(false);
    request["temperature"] = json!(0);
    if let Some(object) = request.as_object_mut() {
        object.remove("stream_options");
    }

    serde_json::to_vec(&request).ok().map(Bytes::from)
}

pub(super) fn build_structured_output_retry_request(
    request_body: &Bytes,
    issue: Option<StructuredOutputIssue>,
) -> Option<Bytes> {
    let mut request = serde_json::from_slice::<Value>(request_body).ok()?;
    let suffix = retry_suffix_for(issue);
    let messages = request.get_mut("messages")?.as_array_mut()?;
    let last_user = messages.iter_mut().rev().find(|message| {
        message.get("role").and_then(Value::as_str) == Some("user")
    })?;
    append_retry_suffix(last_user.get_mut("content")?, suffix)?;
    request["stream"] = Value::Bool(false);
    if let Some(object) = request.as_object_mut() {
        object.remove("stream_options");
    }

    serde_json::to_vec(&request).ok().map(Bytes::from)
}

fn append_retry_suffix(content: &mut Value, suffix: &str) -> Option<()> {
    match content {
        Value::String(text) => {
            text.push_str(suffix);
            Some(())
        }
        Value::Array(parts) => {
            let text_part = parts.iter_mut().rev().find(|part| {
                part.get("type").and_then(Value::as_str) == Some("text")
                    && part.get("text").is_some()
            })?;
            let mut updated = text_part.get("text")?.as_str()?.to_string();
            updated.push_str(suffix);
            text_part["text"] = Value::String(updated);
            Some(())
        }
        _ => None,
    }
}

fn assistant_content(response_body: &Bytes) -> Option<String> {
    let response = serde_json::from_slice::<Value>(response_body).ok()?;
    response
        .pointer("/choices/0/message/content")
        .and_then(Value::as_str)
        .map(ToString::to_string)
        .filter(|content| !content.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{model_id::ModelId, provider::InferenceProvider};

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
            json_schema_rank: 0,
            intent_tier: crate::router::intent::IntentTier::FastThinking,
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
    fn normalizes_single_markdown_fenced_json_response() {
        let response = Bytes::from(
            r#"{"choices":[{"message":{"content":"```json\n{\"status\":\"ok\"}\n```"}}]}"#,
        );

        let normalized =
            normalize_markdown_fenced_json_response(&response).unwrap();
        let value: Value = serde_json::from_slice(&normalized).unwrap();

        assert_eq!(
            value["choices"][0]["message"]["content"],
            r#"{"status":"ok"}"#
        );
    }

    #[test]
    fn does_not_normalize_prose_wrapped_fenced_json() {
        let response = Bytes::from(
            r#"{"choices":[{"message":{"content":"Here:\n```json\n{\"status\":\"ok\"}\n```"}}]}"#,
        );

        assert!(normalize_markdown_fenced_json_response(&response).is_none());
    }

    #[test]
    fn enables_markdown_fenced_json_normalizer_only_for_llm7() {
        assert!(markdown_fenced_json_normalizer_enabled(
            &InferenceProvider::Named("llm7".into())
        ));
        assert!(!markdown_fenced_json_normalizer_enabled(
            &InferenceProvider::Named("longcat".into())
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
        let response =
            Bytes::from(r#"{"choices":[{"message":{"content":""}}]}"#);
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
        let response =
            Bytes::from(r#"{"choices":[{"message":{"content":"not json"}}]}"#);
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
        let response =
            Bytes::from(r#"{"choices":[{"message":{"content":"nope"}}]}"#);
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

    #[test]
    fn builds_schema_conformance_reflection_request() {
        let request = json_schema_request(true, true);
        let response = Bytes::from(
            r#"{"choices":[{"message":{"content":"```json\n{\"status\":42}\n```"}}]}"#,
        );

        let repaired =
            build_schema_conformance_reflection_request(&request, &response)
                .expect("reflection request");
        let value: Value = serde_json::from_slice(&repaired).unwrap();

        assert_eq!(value["stream"], Value::Bool(false));
        assert_eq!(value["temperature"], json!(0));
        assert_eq!(
            value["response_format"]["json_schema"]["schema"]["required"][0],
            "status"
        );
        let messages = value["messages"].as_array().unwrap();
        assert_eq!(messages.len(), 2);
        assert!(
            messages[0]["content"]
                .as_str()
                .unwrap()
                .contains("JSON Schema conformance reflector")
        );
        assert!(
            messages[1]["content"]
                .as_str()
                .unwrap()
                .contains(r#"{"status":42}"#)
        );
    }

    #[test]
    fn enables_reflector_only_for_llm7_and_longcat() {
        assert!(schema_conformance_reflector_enabled(
            &InferenceProvider::Named("llm7".into())
        ));
        assert!(schema_conformance_reflector_enabled(
            &InferenceProvider::Named("longcat".into())
        ));
        assert!(!schema_conformance_reflector_enabled(
            &InferenceProvider::OpenRouter
        ));
    }
}
