//! Prompt injection for `ChatGPT` web — `OpenAI` `response_format` has no
//! native equivalent on chatgpt.com, so we prepend schema instructions to the
//! system message.

use async_openai::types::chat::{
    ChatCompletionRequestMessage, ChatCompletionRequestSystemMessage,
    ChatCompletionRequestSystemMessageContent, CreateChatCompletionRequest,
    ResponseFormat,
};

const ONLY_JSON_TAIL: &str = "Output ONLY the JSON object in the message \
                              content. No markdown fences, no prose.";

const STRICT_MANDATORY: &str =
    "MANDATORY strict mode: you MUST NOT omit any required field, use wrong \
     types, or add extra properties. The response must be a single JSON \
     object that validates against the schema below — no exceptions.";

/// Builds the upstream instruction when structured output is requested.
#[must_use]
pub fn build_json_schema_instruction(
    format: &ResponseFormat,
) -> Option<String> {
    let schema_text = match format {
        ResponseFormat::JsonSchema { json_schema } => {
            let mut lines = Vec::new();
            if json_schema.strict.unwrap_or(false) {
                lines.push(STRICT_MANDATORY.into());
            }
            let schema = json_schema.schema.as_ref().map_or_else(
                || "{}".into(),
                |s| {
                    serde_json::to_string_pretty(s)
                        .unwrap_or_else(|_| "{}".into())
                },
            );
            lines.push(format!(
                "You must respond with valid JSON that strictly follows this \
                 JSON schema:\n{schema}"
            ));
            lines.push(ONLY_JSON_TAIL.into());
            lines.join("\n")
        }
        ResponseFormat::JsonObject => format!(
            "You must respond with a valid JSON object.\n{ONLY_JSON_TAIL}"
        ),
        ResponseFormat::Text => return None,
    };
    Some(schema_text)
}

pub fn inject_json_schema(request: &mut CreateChatCompletionRequest) {
    let Some(instruction) = request
        .response_format
        .as_ref()
        .and_then(build_json_schema_instruction)
    else {
        return;
    };

    if let Some(ChatCompletionRequestMessage::System(system)) = request
        .messages
        .iter_mut()
        .find(|m| matches!(m, ChatCompletionRequestMessage::System(_)))
    {
        match &mut system.content {
            ChatCompletionRequestSystemMessageContent::Text(t) => {
                t.push('\n');
                t.push_str(&instruction);
            }
            ChatCompletionRequestSystemMessageContent::Array(parts) => {
                parts.push(
                    async_openai::types::chat::ChatCompletionRequestSystemMessageContentPart::Text(
                        async_openai::types::chat::ChatCompletionRequestMessageContentPartText {
                            text: instruction,
                        },
                    ),
                );
            }
        }
    } else {
        request.messages.insert(
            0,
            ChatCompletionRequestMessage::System(
                ChatCompletionRequestSystemMessage {
                    content: ChatCompletionRequestSystemMessageContent::Text(
                        instruction,
                    ),
                    name: None,
                },
            ),
        );
    }
}

#[cfg(test)]
mod tests {
    use async_openai::types::chat::ResponseFormatJsonSchema;
    use serde_json::json;

    use super::*;

    fn strict_schema_request() -> CreateChatCompletionRequest {
        CreateChatCompletionRequest {
            model: "gpt-5-mini".into(),
            messages: vec![],
            response_format: Some(ResponseFormat::JsonSchema {
                json_schema: ResponseFormatJsonSchema {
                    name: "entity".into(),
                    description: None,
                    schema: Some(json!({
                        "type": "object",
                        "properties": { "name": { "type": "string" } },
                        "required": ["name"],
                        "additionalProperties": false
                    })),
                    strict: Some(true),
                },
            }),
            ..Default::default()
        }
    }

    #[test]
    fn strict_json_schema_instruction_contains_schema_and_only_json() {
        let format = ResponseFormat::JsonSchema {
            json_schema: ResponseFormatJsonSchema {
                name: "entity".into(),
                description: None,
                schema: Some(json!({
                    "type": "object",
                    "properties": { "name": { "type": "string" } },
                    "required": ["name"],
                    "additionalProperties": false
                })),
                strict: Some(true),
            },
        };
        let instruction = build_json_schema_instruction(&format).unwrap();
        assert!(instruction.contains("MANDATORY strict mode"));
        assert!(instruction.contains("\"name\""));
        assert!(instruction.contains(ONLY_JSON_TAIL));
    }

    #[test]
    fn injects_new_system_message_when_missing() {
        let mut req = strict_schema_request();
        inject_json_schema(&mut req);
        assert_eq!(req.messages.len(), 1);
        let ChatCompletionRequestMessage::System(sys) = &req.messages[0] else {
            panic!("expected system message");
        };
        let ChatCompletionRequestSystemMessageContent::Text(text) =
            &sys.content
        else {
            panic!("expected text system content");
        };
        assert!(text.contains(ONLY_JSON_TAIL));
        assert!(text.contains("MANDATORY strict mode"));
    }

    #[test]
    fn appends_to_existing_system_message() {
        let mut req = strict_schema_request();
        req.messages.push(ChatCompletionRequestMessage::System(
            ChatCompletionRequestSystemMessage {
                content: ChatCompletionRequestSystemMessageContent::Text(
                    "You are a helpful assistant.".into(),
                ),
                name: None,
            },
        ));
        inject_json_schema(&mut req);
        let ChatCompletionRequestMessage::System(sys) = &req.messages[0] else {
            panic!("expected system");
        };
        let ChatCompletionRequestSystemMessageContent::Text(text) =
            &sys.content
        else {
            panic!("expected text");
        };
        assert!(text.starts_with("You are a helpful assistant."));
        assert!(text.contains(ONLY_JSON_TAIL));
    }

    #[test]
    fn text_response_format_does_not_inject() {
        let mut req = CreateChatCompletionRequest {
            response_format: Some(ResponseFormat::Text),
            ..Default::default()
        };
        inject_json_schema(&mut req);
        assert!(req.messages.is_empty());
    }

    #[test]
    fn json_object_injects_only_json_instruction() {
        let instruction =
            build_json_schema_instruction(&ResponseFormat::JsonObject).unwrap();
        assert!(instruction.contains(ONLY_JSON_TAIL));
        assert!(!instruction.contains("MANDATORY strict mode"));
    }
}
