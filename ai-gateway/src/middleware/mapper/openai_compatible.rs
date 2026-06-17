use std::str::FromStr;

use async_openai::types::chat::{
    CreateChatCompletionResponse, CreateChatCompletionStreamResponse,
};
use http::response::Parts;
use serde_json::Value;

use super::{TryConvertStreamData, model::ModelMapper};
use crate::{
    endpoints::openai::OpenAICompatibleChatCompletionRequest,
    error::mapper::MapperError,
    middleware::mapper::{
        TryConvert, TryConvertError, openai_chat_response,
        openai_error_from_value,
    },
    types::{model_id::ModelId, provider::InferenceProvider},
};

pub struct OpenAICompatibleConverter {
    provider: InferenceProvider,
    model_mapper: ModelMapper,
}

impl OpenAICompatibleConverter {
    #[must_use]
    pub fn new(provider: InferenceProvider, model_mapper: ModelMapper) -> Self {
        Self {
            provider,
            model_mapper,
        }
    }
}

impl
    TryConvert<
        async_openai::types::chat::CreateChatCompletionRequest,
        OpenAICompatibleChatCompletionRequest,
    > for OpenAICompatibleConverter
{
    type Error = MapperError;
    fn try_convert(
        &self,
        mut value: async_openai::types::chat::CreateChatCompletionRequest,
    ) -> Result<OpenAICompatibleChatCompletionRequest, Self::Error> {
        let source_model = ModelId::from_str(&value.model)?;
        let target_model =
            self.model_mapper.map_model(&source_model, &self.provider)?;
        tracing::trace!(source_model = ?source_model, target_model = ?target_model, "mapped model");
        value.model = target_model.to_string();

        Ok(OpenAICompatibleChatCompletionRequest {
            provider: self.provider.clone(),
            inner: value,
        })
    }
}

impl TryConvert<CreateChatCompletionResponse, CreateChatCompletionResponse>
    for OpenAICompatibleConverter
{
    type Error = MapperError;
    fn try_convert(
        &self,
        value: CreateChatCompletionResponse,
    ) -> Result<CreateChatCompletionResponse, Self::Error> {
        Ok(value)
    }
}

impl TryConvert<Value, CreateChatCompletionResponse>
    for OpenAICompatibleConverter
{
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

impl
    TryConvertStreamData<
        CreateChatCompletionStreamResponse,
        CreateChatCompletionStreamResponse,
    > for OpenAICompatibleConverter
{
    type Error = MapperError;

    fn try_convert_chunk(
        &self,
        value: CreateChatCompletionStreamResponse,
    ) -> Result<Option<CreateChatCompletionStreamResponse>, Self::Error> {
        Ok(Some(value))
    }
}

impl TryConvertStreamData<Value, CreateChatCompletionStreamResponse>
    for OpenAICompatibleConverter
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
    for OpenAICompatibleConverter
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::middleware::mapper::openai_chat_response;

    #[test]
    fn normalizes_array_content_before_deserialize() {
        let mut value = serde_json::json!({
            "id": "chatcmpl-test",
            "object": "chat.completion",
            "created": 0,
            "model": "gpt-4",
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": [{"type": "text", "text": "hello"}]
                },
                "finish_reason": "stop"
            }]
        });
        openai_chat_response::normalize_chat_completion(&mut value);
        openai_chat_response::ensure_non_empty_choices(&value).unwrap();
        let response: CreateChatCompletionResponse =
            serde_json::from_value(value).expect("array content");
        assert_eq!(
            response.choices[0].message.content.as_ref().unwrap(),
            "hello"
        );
    }
}
