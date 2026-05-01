use async_openai::types::chat as openai;
use uuid::Uuid;

use crate::{
    endpoints::bedrock::converse::BedrockConverseResponse,
    error::mapper::MapperError, middleware::mapper::TryConvert,
};

impl TryConvert<BedrockConverseResponse, openai::CreateChatCompletionResponse>
    for super::BedrockConverter
{
    type Error = MapperError;
    fn try_convert(
        &self,
        value: BedrockConverseResponse,
    ) -> Result<openai::CreateChatCompletionResponse, Self::Error> {
        let payload = &value.payload;
        let model = payload
            .pointer("/trace/promptRouter/invokedModelId")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();
        let usage = map_usage(payload);
        let (content, tool_calls) = map_output(payload);

        let message = openai::ChatCompletionResponseMessage {
            content,
            refusal: None,
            tool_calls,
            role: openai::Role::Assistant,
            #[allow(deprecated)]
            function_call: None,
            audio: None,
            annotations: None,
        };

        Ok(openai::CreateChatCompletionResponse {
            choices: vec![openai::ChatChoice { index: 0, message, finish_reason: None, logprobs: None }],
            id: Uuid::new_v4().to_string(),
            created: 0,
            model,
            object: crate::middleware::mapper::anthropic::OPENAI_CHAT_COMPLETION_OBJECT.to_string(),
            usage: Some(usage),
            service_tier: None,
            #[allow(deprecated)]
            system_fingerprint: None,
        })
    }
}

fn map_usage(payload: &serde_json::Value) -> openai::CompletionUsage {
    let usage = payload.get("usage");
    let input_tokens = usage
        .and_then(|u| u.get("inputTokens"))
        .and_then(serde_json::Value::as_u64)
        .map_or(0, |v| v as u32);
    let output_tokens = usage
        .and_then(|u| u.get("outputTokens"))
        .and_then(serde_json::Value::as_u64)
        .map_or(0, |v| v as u32);
    let total_tokens = usage
        .and_then(|u| u.get("totalTokens"))
        .and_then(serde_json::Value::as_u64)
        .map_or(0, |v| v as u32);

    openai::CompletionUsage {
        prompt_tokens: input_tokens,
        completion_tokens: output_tokens,
        total_tokens,
        prompt_tokens_details: None,
        completion_tokens_details: None,
    }
}

fn map_output(
    payload: &serde_json::Value,
) -> (
    Option<String>,
    Option<Vec<openai::ChatCompletionMessageToolCalls>>,
) {
    let mut tool_calls = Vec::new();
    let mut content = None;

    if let Some(contents) = payload
        .pointer("/output/message/content")
        .and_then(|v| v.as_array())
    {
        for block in contents {
            if let Some(text) = block.get("text").and_then(|v| v.as_str()) {
                content = Some(text.to_string());
            } else if let Some(tool_use) = block.get("toolUse") {
                let id = tool_use
                    .get("toolUseId")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string();
                let name = tool_use
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string();
                let arguments = tool_use
                    .get("input")
                    .map(std::string::ToString::to_string)
                    .unwrap_or_default();

                tool_calls.push(
                    openai::ChatCompletionMessageToolCalls::Function(
                        openai::ChatCompletionMessageToolCall {
                            id,
                            function: openai::FunctionCall { name, arguments },
                        },
                    ),
                );
            }
        }
    }
    (
        content,
        if tool_calls.is_empty() {
            None
        } else {
            Some(tool_calls)
        },
    )
}
