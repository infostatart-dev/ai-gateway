use anthropic_ai_sdk::types::message as anthropic;
use async_openai::types::chat as openai;

use crate::{
    error::mapper::MapperError, middleware::mapper::TryConvertStreamData,
};

impl
    TryConvertStreamData<
        openai::CreateChatCompletionStreamResponse,
        anthropic::StreamEvent,
    > for super::OpenAIConverter
{
    type Error = MapperError;

    #[allow(clippy::too_many_lines)]
    fn try_convert_chunk(
        &self,
        value: openai::CreateChatCompletionStreamResponse,
    ) -> Result<Option<anthropic::StreamEvent>, Self::Error> {
        let Some(first_choice) = value.choices.first() else {
            return Ok(None);
        };
        let delta = &first_choice.delta;

        if let Some(role) = delta.role {
            let mut content = Vec::new();
            if let Some(text) = &delta.content {
                content
                    .push(anthropic::ContentBlock::Text { text: text.clone() });
            }
            if let Some(tool_calls) = &delta.tool_calls {
                for tc in tool_calls {
                    if let (Some(id), Some(func)) =
                        (tc.id.as_ref(), tc.function.as_ref())
                        && let Some(name) = &func.name
                    {
                        let input = serde_json::from_str(
                            func.arguments.as_deref().unwrap_or("{}"),
                        )
                        .unwrap_or(serde_json::json!({}));
                        content.push(anthropic::ContentBlock::ToolUse {
                            id: id.clone(),
                            name: name.clone(),
                            input,
                        });
                    }
                }
            }
            return Ok(Some(anthropic::StreamEvent::MessageStart {
                message: anthropic::MessageStartContent {
                    id: value.id,
                    type_: "message".to_string(),
                    role: if matches!(role, openai::Role::Assistant) {
                        anthropic::Role::Assistant
                    } else {
                        anthropic::Role::User
                    },
                    content,
                    model: value.model,
                    stop_reason: None,
                    stop_sequence: None,
                    usage: anthropic::Usage {
                        input_tokens: 0,
                        output_tokens: 0,
                    },
                },
            }));
        }

        if let Some(reason) = first_choice.finish_reason {
            let stop_reason = match reason {
                openai::FinishReason::Stop => anthropic::StopReason::EndTurn,
                openai::FinishReason::Length => {
                    anthropic::StopReason::MaxTokens
                }
                openai::FinishReason::ToolCalls
                | openai::FinishReason::FunctionCall => {
                    anthropic::StopReason::ToolUse
                }
                openai::FinishReason::ContentFilter => {
                    anthropic::StopReason::Refusal
                }
            };
            let usage = value.usage.map_or(
                anthropic::StreamUsage {
                    input_tokens: 0,
                    output_tokens: 0,
                },
                |u| anthropic::StreamUsage {
                    input_tokens: u.prompt_tokens,
                    output_tokens: u.completion_tokens,
                },
            );
            return Ok(Some(anthropic::StreamEvent::MessageDelta {
                delta: anthropic::MessageDeltaContent {
                    stop_reason: Some(stop_reason),
                    stop_sequence: None,
                },
                usage: Some(usage),
            }));
        }

        if let Some(tool_calls) = &delta.tool_calls
            && let Some(tc) = tool_calls.first()
        {
            if let (Some(id), Some(func), Some(name)) = (
                tc.id.as_ref(),
                tc.function.as_ref(),
                tc.function.as_ref().and_then(|f| f.name.as_ref()),
            ) {
                let input = serde_json::from_str(
                    func.arguments.as_deref().unwrap_or("{}"),
                )
                .unwrap_or(serde_json::json!({}));
                return Ok(Some(anthropic::StreamEvent::ContentBlockStart {
                    index: tc.index as usize,
                    content_block: anthropic::ContentBlock::ToolUse {
                        id: id.clone(),
                        name: name.clone(),
                        input,
                    },
                }));
            } else if let Some(args) =
                tc.function.as_ref().and_then(|f| f.arguments.as_ref())
            {
                return Ok(Some(anthropic::StreamEvent::ContentBlockDelta {
                    index: tc.index as usize,
                    delta: anthropic::ContentBlockDelta::InputJsonDelta {
                        partial_json: args.clone(),
                    },
                }));
            }
        }

        if let Some(text) = &delta.content {
            return Ok(Some(anthropic::StreamEvent::ContentBlockDelta {
                index: 0,
                delta: anthropic::ContentBlockDelta::TextDelta {
                    text: text.clone(),
                },
            }));
        }

        Ok(None)
    }
}

impl
    TryConvertStreamData<
        openai::CreateChatCompletionStreamResponse,
        openai::CreateChatCompletionStreamResponse,
    > for super::OpenAIConverter
{
    type Error = MapperError;
    fn try_convert_chunk(
        &self,
        value: openai::CreateChatCompletionStreamResponse,
    ) -> Result<Option<openai::CreateChatCompletionStreamResponse>, Self::Error>
    {
        Ok(Some(value))
    }
}
