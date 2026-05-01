use std::str::FromStr;

use async_openai::types::chat::{
    CreateChatCompletionRequest, CreateChatCompletionResponse,
    CreateChatCompletionStreamResponse,
};
use http::response::Parts;

use super::{TryConvert, TryConvertStreamData};
use crate::{
    endpoints::openrouter::chat_completions::CreateChatCompletionRequestOpenRouter,
    error::mapper::MapperError,
    middleware::mapper::{
        TryConvertError, model::ModelMapper, openai_error_from_value,
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

        // Convert via JSON to adapt from async_openai structure to
        // openrouter-rs cleanly
        let mut value_json = serde_json::to_value(&value)
            .map_err(|e| MapperError::UnsupportedFormat(e.to_string()))?;
        if let Some(obj) = value_json.as_object_mut() {
            obj.insert(
                "model".to_string(),
                serde_json::Value::String(target_model.to_string()),
            );
        }

        Ok(CreateChatCompletionRequestOpenRouter(value_json))
    }
}

impl TryConvert<CreateChatCompletionResponse, CreateChatCompletionResponse>
    for OpenRouterConverter
{
    type Error = MapperError;
    fn try_convert(
        &self,
        value: CreateChatCompletionResponse,
    ) -> Result<CreateChatCompletionResponse, Self::Error> {
        Ok(value)
    }
}

impl
    TryConvertStreamData<
        CreateChatCompletionStreamResponse,
        CreateChatCompletionStreamResponse,
    > for OpenRouterConverter
{
    type Error = MapperError;

    fn try_convert_chunk(
        &self,
        value: CreateChatCompletionStreamResponse,
    ) -> Result<Option<CreateChatCompletionStreamResponse>, Self::Error> {
        Ok(Some(value))
    }
}

impl TryConvertError<serde_json::Value, async_openai::error::WrappedError>
    for OpenRouterConverter
{
    type Error = MapperError;

    fn try_convert_error(
        &self,
        resp_parts: &Parts,
        value: serde_json::Value,
    ) -> Result<async_openai::error::WrappedError, Self::Error> {
        Ok(openai_error_from_value(resp_parts.status, value))
    }
}
