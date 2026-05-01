use anthropic_ai_sdk::types::message as anthropic;
use async_openai::types::chat as openai;

use super::OPENAI_CHAT_COMPLETION_OBJECT;
use crate::{error::mapper::MapperError, middleware::mapper::TryConvert};

impl
    TryConvert<
        anthropic::CreateMessageResponse,
        openai::CreateChatCompletionResponse,
    > for super::AnthropicConverter
{
    type Error = MapperError;

    fn try_convert(
        &self,
        value: anthropic::CreateMessageResponse,
    ) -> Result<openai::CreateChatCompletionResponse, Self::Error> {
        let id = value.id;
        let model = value.model;
        let usage = openai::CompletionUsage {
            prompt_tokens: value.usage.input_tokens,
            completion_tokens: value.usage.output_tokens,
            total_tokens: value.usage.input_tokens + value.usage.output_tokens,
            prompt_tokens_details: None,
            completion_tokens_details: None,
        };

        let mut tool_calls = Vec::new();
        let mut content = None;
        for anthropic_content in value.content {
            match anthropic_content {
                anthropic::ContentBlock::ToolUse { id, name, input } => {
                    tool_calls.push(
                        openai::ChatCompletionMessageToolCalls::Function(
                            openai::ChatCompletionMessageToolCall {
                                id: id.clone(),
                                function: openai::FunctionCall {
                                    name: name.clone(),
                                    arguments: serde_json::to_string(&input)?,
                                },
                            },
                        ),
                    );
                }
                anthropic::ContentBlock::ToolResult {
                    tool_use_id,
                    content: tool_content,
                } => {
                    tool_calls.push(
                        openai::ChatCompletionMessageToolCalls::Function(
                            openai::ChatCompletionMessageToolCall {
                                id: tool_use_id.clone(),
                                function: openai::FunctionCall {
                                    name: tool_use_id.clone(),
                                    arguments: serde_json::to_string(
                                        &tool_content,
                                    )?,
                                },
                            },
                        ),
                    );
                }
                anthropic::ContentBlock::Text { text, .. } => {
                    content = Some(text.clone());
                }
                _ => {}
            }
        }

        let message = openai::ChatCompletionResponseMessage {
            content,
            refusal: None,
            tool_calls: if tool_calls.is_empty() {
                None
            } else {
                Some(tool_calls)
            },
            role: openai::Role::Assistant,
            #[allow(deprecated)]
            function_call: None,
            audio: None,
            annotations: None,
        };

        Ok(openai::CreateChatCompletionResponse {
            choices: vec![openai::ChatChoice {
                index: 0,
                message,
                finish_reason: None,
                logprobs: None,
            }],
            id,
            created: 0,
            model,
            object: OPENAI_CHAT_COMPLETION_OBJECT.to_string(),
            usage: Some(usage),
            service_tier: None,
            #[allow(deprecated)]
            system_fingerprint: None,
        })
    }
}

impl
    TryConvert<
        anthropic::CreateMessageResponse,
        anthropic::CreateMessageResponse,
    > for super::AnthropicConverter
{
    type Error = MapperError;
    fn try_convert(
        &self,
        value: anthropic::CreateMessageResponse,
    ) -> Result<anthropic::CreateMessageResponse, Self::Error> {
        Ok(value)
    }
}
