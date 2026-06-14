use std::str::FromStr;

use async_openai::types::chat::{
    CreateChatCompletionResponse, CreateChatCompletionStreamResponse,
};
use http::response::Parts;
use serde_json::Value;

use crate::{
    endpoints::{Endpoint, openai::OpenAICompatibleChatCompletionRequest},
    error::mapper::MapperError,
    middleware::mapper::{
        TryConvert, TryConvertError, TryConvertStreamData, model::ModelMapper,
        openai_chat_response, openai_error_from_value,
    },
    types::{model_id::ModelId, provider::InferenceProvider},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct CloudflareChatCompletions;

impl Endpoint for CloudflareChatCompletions {
    const PATH: &'static str = "v1/chat/completions";
    type RequestBody = OpenAICompatibleChatCompletionRequest;
    type ResponseBody = Value;
    type StreamResponseBody = Value;
    type ErrorResponseBody = Value;
}

pub struct CloudflareConverter {
    model_mapper: ModelMapper,
}

impl CloudflareConverter {
    #[must_use]
    pub fn new(model_mapper: ModelMapper) -> Self {
        Self { model_mapper }
    }
}

impl
    TryConvert<
        async_openai::types::chat::CreateChatCompletionRequest,
        OpenAICompatibleChatCompletionRequest,
    > for CloudflareConverter
{
    type Error = MapperError;
    fn try_convert(
        &self,
        mut value: async_openai::types::chat::CreateChatCompletionRequest,
    ) -> Result<OpenAICompatibleChatCompletionRequest, Self::Error> {
        let source_model = ModelId::from_str(&value.model)?;
        let target_model = self.model_mapper.map_model(
            &source_model,
            &InferenceProvider::Named("cloudflare".into()),
        )?;
        value.model = target_model.to_string();
        flatten_message_content(&mut value.messages);
        Ok(OpenAICompatibleChatCompletionRequest {
            provider: InferenceProvider::Named("cloudflare".into()),
            inner: value,
        })
    }
}

impl TryConvert<Value, CreateChatCompletionResponse> for CloudflareConverter {
    type Error = MapperError;
    fn try_convert(
        &self,
        mut value: Value,
    ) -> Result<CreateChatCompletionResponse, Self::Error> {
        openai_chat_response::normalize_chat_completion(&mut value);
        openai_chat_response::ensure_non_empty_choices(&value)?;
        serde_json::from_value(value).map_err(MapperError::SerdeError)
    }
}

impl TryConvertStreamData<Value, CreateChatCompletionStreamResponse>
    for CloudflareConverter
{
    type Error = MapperError;
    fn try_convert_chunk(
        &self,
        mut value: Value,
    ) -> Result<Option<CreateChatCompletionStreamResponse>, Self::Error> {
        openai_chat_response::normalize_stream_chunk(&mut value);
        let chunk: CreateChatCompletionStreamResponse =
            serde_json::from_value(value).map_err(MapperError::SerdeError)?;
        Ok(Some(chunk))
    }
}

impl TryConvertError<Value, async_openai::error::WrappedError>
    for CloudflareConverter
{
    type Error = MapperError;
    fn try_convert_error(
        &self,
        resp_parts: &Parts,
        value: Value,
    ) -> Result<async_openai::error::WrappedError, Self::Error> {
        Ok(openai_error_from_value(resp_parts.status, &value))
    }
}

fn flatten_message_content(
    messages: &mut [async_openai::types::chat::ChatCompletionRequestMessage],
) {
    for message in messages.iter_mut() {
        let async_openai::types::chat::ChatCompletionRequestMessage::User(user) =
            message
        else {
            continue;
        };
        let async_openai::types::chat::ChatCompletionRequestUserMessageContent::Array(
            parts,
        ) = &mut user.content
        else {
            continue;
        };
        let text = parts
            .iter()
            .filter_map(|part| match part {
                async_openai::types::chat::ChatCompletionRequestUserMessageContentPart::Text(
                    text,
                ) => Some(text.text.clone()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("");
        user.content =
            async_openai::types::chat::ChatCompletionRequestUserMessageContent::Text(
                text,
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_cloudflare_object_content_to_openai_string() {
        let mut value = serde_json::json!({
            "model": "@cf/deepseek-ai/deepseek-r1-distill-qwen-32b",
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": {"type": "text", "text": "reasoned answer"}
                },
                "finish_reason": "stop"
            }]
        });

        openai_chat_response::normalize_chat_completion(&mut value);
        openai_chat_response::ensure_non_empty_choices(&value).unwrap();
        let response: CreateChatCompletionResponse =
            serde_json::from_value(value).expect("cloudflare map content must deserialize");
        assert_eq!(
            response.choices[0].message.content.as_ref().unwrap(),
            "reasoned answer"
        );
    }
}
