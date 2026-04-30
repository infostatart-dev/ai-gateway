use crate::{
    endpoints::bedrock::converse::BedrockConverseStreamOutput,
    error::mapper::MapperError, middleware::mapper::TryConvertStreamData,
};
use async_openai::types::chat as openai;
use uuid::Uuid;

impl
    TryConvertStreamData<
        BedrockConverseStreamOutput,
        openai::CreateChatCompletionStreamResponse,
    > for super::BedrockConverter
{
    type Error = MapperError;
    fn try_convert_chunk(
        &self,
        value: BedrockConverseStreamOutput,
    ) -> Result<Option<openai::CreateChatCompletionStreamResponse>, Self::Error>
    {
        let payload = &value.payload;
        let mut choices = Vec::new();

        if let Some(message_start) = payload.get("messageStart") {
            let role = match message_start.get("role").and_then(|r| r.as_str())
            {
                Some("assistant") => openai::Role::Assistant,
                Some("user") => openai::Role::User,
                _ => openai::Role::System,
            };
            let delta = openai::ChatCompletionStreamResponseDelta {
                role: Some(role),
                content: None,
                tool_calls: None,
                refusal: None,
                #[allow(deprecated)]
                function_call: None,
            };
            choices.push(map_choice(delta, None));
        }

        if let Some(tool_use) =
            payload.pointer("/contentBlockStart/start/toolUse")
        {
            let delta = openai::ChatCompletionStreamResponseDelta {
                role: None,
                content: None,
                refusal: None,
                tool_calls: Some(vec![map_tool_chunk(
                    tool_use,
                    payload.pointer("/contentBlockStart/contentBlockIndex"),
                )]),
                #[allow(deprecated)]
                function_call: None,
            };
            choices.push(map_choice(delta, None));
        }

        if let Some(delta_payload) = payload.get("contentBlockDelta") {
            let index = delta_payload
                .get("contentBlockIndex")
                .and_then(|v| v.as_u64())
                .map(|v| v as u32)
                .unwrap_or(0);
            if let Some(text) = delta_payload.pointer("/delta/text") {
                let delta = openai::ChatCompletionStreamResponseDelta {
                    role: None,
                    content: Some(
                        text.as_str().unwrap_or_default().to_string(),
                    ),
                    tool_calls: None,
                    refusal: None,
                    #[allow(deprecated)]
                    function_call: None,
                };
                choices.push(map_choice_with_index(delta, None, index));
            } else if let Some(tool_use) =
                delta_payload.pointer("/delta/toolUse")
            {
                let delta = openai::ChatCompletionStreamResponseDelta {
                    role: None,
                    content: None,
                    refusal: None,
                    tool_calls: Some(vec![map_tool_delta(tool_use, index)]),
                    #[allow(deprecated)]
                    function_call: None,
                };
                choices.push(map_choice(delta, None));
            }
        }

        let usage = map_usage(payload);
        if choices.is_empty() && usage.is_none() {
            return Ok(None);
        }

        Ok(Some(openai::CreateChatCompletionStreamResponse {
            id: Uuid::new_v4().to_string(),
            choices,
            created: 0,
            model: "bedrock-stream".to_string(),
            object: "chat.completion.chunk".to_string(),
            service_tier: None,
            usage,
            #[allow(deprecated)]
            system_fingerprint: None,
        }))
    }
}

fn map_choice(
    delta: openai::ChatCompletionStreamResponseDelta,
    finish_reason: Option<openai::FinishReason>,
) -> openai::ChatChoiceStream {
    openai::ChatChoiceStream {
        index: 0,
        delta,
        finish_reason,
        logprobs: None,
    }
}

fn map_choice_with_index(
    delta: openai::ChatCompletionStreamResponseDelta,
    finish_reason: Option<openai::FinishReason>,
    index: u32,
) -> openai::ChatChoiceStream {
    openai::ChatChoiceStream {
        index,
        delta,
        finish_reason,
        logprobs: None,
    }
}

fn map_tool_chunk(
    tool_use: &serde_json::Value,
    index_val: Option<&serde_json::Value>,
) -> openai::ChatCompletionMessageToolCallChunk {
    let id = tool_use
        .get("toolUseId")
        .and_then(|v| v.as_str())
        .map(|v| v.to_string());
    let name = tool_use
        .get("name")
        .and_then(|v| v.as_str())
        .map(|v| v.to_string());
    let index = index_val
        .and_then(|v| v.as_u64())
        .map(|v| v as u32)
        .unwrap_or(0);
    openai::ChatCompletionMessageToolCallChunk {
        index,
        id,
        r#type: Some(openai::FunctionType::Function),
        function: Some(openai::FunctionCallStream {
            name,
            arguments: Some(String::new()),
        }),
    }
}

fn map_tool_delta(
    tool_use: &serde_json::Value,
    index: u32,
) -> openai::ChatCompletionMessageToolCallChunk {
    let input = tool_use
        .get("input")
        .and_then(|v| v.as_str())
        .map(|v| v.to_string());
    openai::ChatCompletionMessageToolCallChunk {
        index,
        id: None,
        r#type: Some(openai::FunctionType::Function),
        function: Some(openai::FunctionCallStream {
            name: None,
            arguments: input,
        }),
    }
}

fn map_usage(payload: &serde_json::Value) -> Option<openai::CompletionUsage> {
    payload.pointer("/metadata/usage").map(|u| {
        let input_tokens = u
            .get("inputTokens")
            .and_then(|v| v.as_u64())
            .map(|v| v as u32)
            .unwrap_or(0);
        let output_tokens = u
            .get("outputTokens")
            .and_then(|v| v.as_u64())
            .map(|v| v as u32)
            .unwrap_or(0);
        let total_tokens = u
            .get("totalTokens")
            .and_then(|v| v.as_u64())
            .map(|v| v as u32)
            .unwrap_or(0);
        openai::CompletionUsage {
            prompt_tokens: input_tokens,
            completion_tokens: output_tokens,
            total_tokens,
            prompt_tokens_details: None,
            completion_tokens_details: None,
        }
    })
}
