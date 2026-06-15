use std::str::FromStr;

use async_openai::types::chat::{
    CreateChatCompletionRequest, CreateChatCompletionResponse,
    CreateChatCompletionStreamResponse,
};
use http::response::Parts;
use serde_json::Value;

use super::{TryConvert, TryConvertStreamData};
use crate::{
    endpoints::openrouter::chat_completions::CreateChatCompletionRequestOpenRouter,
    error::mapper::MapperError,
    middleware::mapper::{
        TryConvertError, model::ModelMapper, openai_chat_response,
        openai_error_from_value,
    },
    types::{model_id::ModelId, provider::InferenceProvider},
};

pub struct OpenRouterConverter {
    model_mapper: ModelMapper,
}

impl OpenRouterConverter {
    #[must_use]
    pub fn new(model_mapper: ModelMapper) -> Self {
        Self { model_mapper }
    }
}

impl
    TryConvert<
        CreateChatCompletionRequest,
        CreateChatCompletionRequestOpenRouter,
    > for OpenRouterConverter
{
    type Error = MapperError;
    fn try_convert(
        &self,
        value: CreateChatCompletionRequest,
    ) -> Result<CreateChatCompletionRequestOpenRouter, Self::Error> {
        let source_model = ModelId::from_str(&value.model)?;
        let target_model = self
            .model_mapper
            .map_model(&source_model, &InferenceProvider::OpenRouter)?;
        tracing::trace!(source_model = ?source_model, target_model = ?target_model, "mapped model");

        let mut value_json = serde_json::to_value(&value)
            .map_err(|e| MapperError::UnsupportedFormat(e.to_string()))?;
        if let Some(obj) = value_json.as_object_mut() {
            obj.insert(
                "model".to_string(),
                Value::String(target_model.to_string()),
            );
        }

        Ok(CreateChatCompletionRequestOpenRouter(value_json))
    }
}

impl TryConvert<Value, CreateChatCompletionResponse> for OpenRouterConverter {
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
    for OpenRouterConverter
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
    for OpenRouterConverter
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

    #[test]
    fn normalizes_openrouter_response_missing_openai_required_fields() {
        let mut value = serde_json::json!({
            "model": "openai/gpt-oss-120b:free",
            "choices": [{
                "index": 0,
                "message": {"role": "assistant", "content": "Привет"},
                "finish_reason": "stop"
            }],
            "usage": {
                "prompt_tokens": 10,
                "completion_tokens": 5,
                "total_tokens": 15
            }
        });

        openai_chat_response::normalize_chat_completion(&mut value);
        let response: CreateChatCompletionResponse =
            serde_json::from_value(value)
                .expect("response must deserialize after normalization");

        assert!(response.id.starts_with("chatcmpl-"));
        assert_eq!(response.object, "chat.completion");
        assert_eq!(response.model, "openai/gpt-oss-120b:free");
        assert_eq!(
            response.choices[0].message.content.as_ref().unwrap(),
            "Привет"
        );
    }

    #[test]
    fn rejects_openrouter_response_without_choices() {
        let value = serde_json::json!({
            "model": "openai/gpt-oss-120b:free",
            "usage": {"total_tokens": 0}
        });

        let error = openai_chat_response::ensure_non_empty_choices(&value)
            .expect_err("missing choices must fail before serde");
        assert!(matches!(error, MapperError::UnsupportedFormat(_)));
    }
}
