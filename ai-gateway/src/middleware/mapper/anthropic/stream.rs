use crate::{
    error::mapper::MapperError, middleware::mapper::TryConvertStreamData,
};
use anthropic_ai_sdk::types::message as anthropic;
use async_openai::types::chat as openai;

impl
    TryConvertStreamData<
        anthropic::StreamEvent,
        openai::CreateChatCompletionStreamResponse,
    > for super::AnthropicConverter
{
    type Error = MapperError;

    #[allow(deprecated)]
    fn try_convert_chunk(
        &self,
        value: anthropic::StreamEvent,
    ) -> Result<Option<openai::CreateChatCompletionStreamResponse>, Self::Error>
    {
        const CHAT_COMPLETION_CHUNK_OBJECT: &str = "chat.completion.chunk";
        const PLACEHOLDER_STREAM_ID: &str = "anthropic-stream-id";
        const PLACEHOLDER_MODEL_NAME: &str = "anthropic-model";

        match value {
            anthropic::StreamEvent::MessageStart { message } => {
                let mut current_text_content = String::new();
                let mut tool_calls = Vec::new();

                for (idx, content_block) in message.content.iter().enumerate() {
                    match content_block {
                        anthropic::ContentBlock::Text { text, .. } => {
                            current_text_content.push_str(text)
                        }
                        anthropic::ContentBlock::ToolUse {
                            id,
                            name,
                            input,
                        } => {
                            tool_calls.push(
                                openai::ChatCompletionMessageToolCallChunk {
                                    index: idx as u32,
                                    id: Some(id.clone()),
                                    r#type: Some(
                                        openai::FunctionType::Function,
                                    ),
                                    function: Some(
                                        openai::FunctionCallStream {
                                            name: Some(name.clone()),
                                            arguments: Some(
                                                serde_json::to_string(input)
                                                    .map_err(
                                                        MapperError::SerdeError,
                                                    )?,
                                            ),
                                        },
                                    ),
                                },
                            );
                        }
                        anthropic::ContentBlock::ToolResult {
                            content, ..
                        } => {
                            current_text_content.push('\n');
                            current_text_content.push_str(content);
                        }
                        _ => {}
                    }
                }

                let finish_reason = match message.stop_reason {
                    Some(
                        anthropic::StopReason::EndTurn
                        | anthropic::StopReason::StopSequence,
                    ) => Some(openai::FinishReason::Stop),
                    Some(anthropic::StopReason::MaxTokens) => {
                        Some(openai::FinishReason::Length)
                    }
                    Some(anthropic::StopReason::ToolUse) => {
                        Some(openai::FinishReason::ToolCalls)
                    }
                    Some(anthropic::StopReason::Refusal) => {
                        Some(openai::FinishReason::ContentFilter)
                    }
                    None => None,
                };

                let choice = openai::ChatChoiceStream {
                    index: 0,
                    delta: openai::ChatCompletionStreamResponseDelta {
                        role: Some(match message.role {
                            anthropic::Role::User => openai::Role::User,
                            anthropic::Role::Assistant => {
                                openai::Role::Assistant
                            }
                        }),
                        content: Some(current_text_content),
                        tool_calls: Some(tool_calls),
                        refusal: if matches!(
                            message.stop_reason,
                            Some(anthropic::StopReason::Refusal)
                        ) {
                            message.stop_sequence.clone()
                        } else {
                            None
                        },
                        function_call: None,
                    },
                    finish_reason,
                    logprobs: None,
                };

                Ok(Some(openai::CreateChatCompletionStreamResponse {
                    id: message.id,
                    choices: vec![choice],
                    created: 0,
                    model: message.model,
                    object: CHAT_COMPLETION_CHUNK_OBJECT.to_string(),
                    system_fingerprint: None,
                    service_tier: None,
                    usage: Some(openai::CompletionUsage {
                        prompt_tokens: message.usage.input_tokens,
                        completion_tokens: message.usage.output_tokens,
                        total_tokens: message.usage.input_tokens
                            + message.usage.output_tokens,
                        prompt_tokens_details: None,
                        completion_tokens_details: None,
                    }),
                }))
            }
            anthropic::StreamEvent::ContentBlockStart {
                index,
                content_block,
            } => {
                if let anthropic::ContentBlock::ToolUse { id, name, input } =
                    content_block
                {
                    let tool_call_chunk =
                        openai::ChatCompletionMessageToolCallChunk {
                            index: index as u32,
                            id: Some(id),
                            r#type: Some(openai::FunctionType::Function),
                            function: Some(openai::FunctionCallStream {
                                name: Some(name),
                                arguments: Some(
                                    serde_json::to_string(&input)
                                        .map_err(MapperError::SerdeError)?,
                                ),
                            }),
                        };
                    let choice = openai::ChatChoiceStream {
                        index: 0,
                        delta: openai::ChatCompletionStreamResponseDelta {
                            role: None,
                            content: None,
                            tool_calls: Some(vec![tool_call_chunk]),
                            refusal: None,
                            function_call: None,
                        },
                        finish_reason: None,
                        logprobs: None,
                    };
                    Ok(Some(openai::CreateChatCompletionStreamResponse {
                        id: PLACEHOLDER_STREAM_ID.to_string(),
                        choices: vec![choice],
                        created: 0,
                        model: PLACEHOLDER_MODEL_NAME.to_string(),
                        object: CHAT_COMPLETION_CHUNK_OBJECT.to_string(),
                        system_fingerprint: None,
                        service_tier: None,
                        usage: None,
                    }))
                } else {
                    Ok(None)
                }
            }
            anthropic::StreamEvent::ContentBlockDelta { index, delta } => {
                let delta_msg = match delta {
                    anthropic::ContentBlockDelta::TextDelta { text } => {
                        openai::ChatCompletionStreamResponseDelta {
                            role: None,
                            content: Some(text),
                            tool_calls: None,
                            refusal: None,
                            function_call: None,
                        }
                    }
                    anthropic::ContentBlockDelta::InputJsonDelta {
                        partial_json,
                    } => openai::ChatCompletionStreamResponseDelta {
                        role: None,
                        content: None,
                        refusal: None,
                        function_call: None,
                        tool_calls: Some(vec![
                            openai::ChatCompletionMessageToolCallChunk {
                                index: index as u32,
                                id: None,
                                r#type: Some(openai::FunctionType::Function),
                                function: Some(openai::FunctionCallStream {
                                    name: None,
                                    arguments: Some(partial_json),
                                }),
                            },
                        ]),
                    },
                    _ => return Ok(None),
                };
                Ok(Some(openai::CreateChatCompletionStreamResponse {
                    id: PLACEHOLDER_STREAM_ID.to_string(),
                    choices: vec![openai::ChatChoiceStream {
                        index: 0,
                        delta: delta_msg,
                        finish_reason: None,
                        logprobs: None,
                    }],
                    created: 0,
                    model: PLACEHOLDER_MODEL_NAME.to_string(),
                    object: CHAT_COMPLETION_CHUNK_OBJECT.to_string(),
                    system_fingerprint: None,
                    service_tier: None,
                    usage: None,
                }))
            }
            anthropic::StreamEvent::MessageDelta { delta, usage } => {
                let finish_reason = match delta.stop_reason {
                    Some(
                        anthropic::StopReason::EndTurn
                        | anthropic::StopReason::StopSequence,
                    ) => Some(openai::FinishReason::Stop),
                    Some(anthropic::StopReason::MaxTokens) => {
                        Some(openai::FinishReason::Length)
                    }
                    Some(anthropic::StopReason::ToolUse) => {
                        Some(openai::FinishReason::ToolCalls)
                    }
                    Some(anthropic::StopReason::Refusal) => {
                        Some(openai::FinishReason::ContentFilter)
                    }
                    None => None,
                };
                let completion_usage = usage.map(|u| openai::CompletionUsage {
                    prompt_tokens: u.input_tokens,
                    completion_tokens: u.output_tokens,
                    total_tokens: u.input_tokens + u.output_tokens,
                    prompt_tokens_details: None,
                    completion_tokens_details: None,
                });
                Ok(Some(openai::CreateChatCompletionStreamResponse {
                    id: PLACEHOLDER_STREAM_ID.to_string(),
                    choices: vec![openai::ChatChoiceStream {
                        index: 0,
                        delta: openai::ChatCompletionStreamResponseDelta {
                            role: None,
                            content: None,
                            tool_calls: None,
                            refusal: delta.stop_sequence,
                            function_call: None,
                        },
                        finish_reason,
                        logprobs: None,
                    }],
                    created: 0,
                    model: PLACEHOLDER_MODEL_NAME.to_string(),
                    object: CHAT_COMPLETION_CHUNK_OBJECT.to_string(),
                    system_fingerprint: None,
                    service_tier: None,
                    usage: completion_usage,
                }))
            }
            _ => Ok(None),
        }
    }
}

impl TryConvertStreamData<anthropic::StreamEvent, anthropic::StreamEvent>
    for super::AnthropicConverter
{
    type Error = MapperError;
    fn try_convert_chunk(
        &self,
        value: anthropic::StreamEvent,
    ) -> Result<Option<anthropic::StreamEvent>, Self::Error> {
        Ok(Some(value))
    }
}
