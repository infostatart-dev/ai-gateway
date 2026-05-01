use anthropic_ai_sdk::types::message as anthropic;
use async_openai::types::chat as openai;

use crate::{error::mapper::MapperError, middleware::mapper::TryConvert};

impl
    TryConvert<
        openai::CreateChatCompletionResponse,
        anthropic::CreateMessageResponse,
    > for super::OpenAIConverter
{
    type Error = MapperError;

    fn try_convert(
        &self,
        mut value: openai::CreateChatCompletionResponse,
    ) -> Result<anthropic::CreateMessageResponse, Self::Error> {
        let id = value.id;
        let model = value.model;
        let usage = value.usage.map_or(
            anthropic::Usage {
                input_tokens: 0,
                output_tokens: 0,
            },
            |u| anthropic::Usage {
                input_tokens: u.prompt_tokens,
                output_tokens: u.completion_tokens,
            },
        );

        let choice = value.choices.remove(0);
        let stop_reason = if choice.message.refusal.is_some() {
            Some(anthropic::StopReason::Refusal)
        } else {
            None
        };
        let mut content = Vec::new();

        if let Some(tool_calls) = choice.message.tool_calls {
            for tc in tool_calls {
                if let openai::ChatCompletionMessageToolCalls::Function(f) = tc
                {
                    if let Ok(input) =
                        serde_json::from_str(&f.function.arguments)
                    {
                        content.push(anthropic::ContentBlock::ToolUse {
                            id: f.id,
                            name: f.function.name,
                            input,
                        });
                    }
                }
            }
        }
        if let Some(text) = choice.message.content {
            content.push(anthropic::ContentBlock::Text { text });
        }

        Ok(anthropic::CreateMessageResponse {
            content,
            id,
            model,
            role: anthropic::Role::Assistant,
            stop_reason,
            stop_sequence: None,
            type_: "message".to_string(),
            usage,
        })
    }
}

impl
    TryConvert<
        openai::CreateChatCompletionResponse,
        openai::CreateChatCompletionResponse,
    > for super::OpenAIConverter
{
    type Error = MapperError;
    fn try_convert(
        &self,
        value: openai::CreateChatCompletionResponse,
    ) -> Result<openai::CreateChatCompletionResponse, Self::Error> {
        Ok(value)
    }
}
