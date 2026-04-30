use std::str::FromStr;

use async_openai::types::chat;
use http::response::Parts;

use super::{TryConvertStreamData, model::ModelMapper};
use crate::{
    endpoints::openai::OpenAICompatibleChatCompletionRequest,
    error::mapper::MapperError,
    middleware::mapper::{
        TryConvert, TryConvertError, openai_error_from_value,
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
        chat::CreateChatCompletionRequest,
        OpenAICompatibleChatCompletionRequest,
    > for OpenAICompatibleConverter
{
    type Error = MapperError;
    fn try_convert(
        &self,
        mut value: chat::CreateChatCompletionRequest,
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

impl
    TryConvert<
        chat::CreateChatCompletionResponse,
        chat::CreateChatCompletionResponse,
    > for OpenAICompatibleConverter
{
    type Error = MapperError;
    fn try_convert(
        &self,
        value: chat::CreateChatCompletionResponse,
    ) -> Result<chat::CreateChatCompletionResponse, Self::Error> {
        Ok(value)
    }
}

impl
    TryConvertStreamData<
        chat::CreateChatCompletionStreamResponse,
        chat::CreateChatCompletionStreamResponse,
    > for OpenAICompatibleConverter
{
    type Error = MapperError;

    fn try_convert_chunk(
        &self,
        value: chat::CreateChatCompletionStreamResponse,
    ) -> Result<Option<chat::CreateChatCompletionStreamResponse>, Self::Error>
    {
        Ok(Some(value))
    }
}

impl TryConvertError<serde_json::Value, async_openai::error::WrappedError>
    for OpenAICompatibleConverter
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
