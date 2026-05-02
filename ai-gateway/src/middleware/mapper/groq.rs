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
        openai_error_from_value,
    },
    types::{model_id::ModelId, provider::InferenceProvider},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct GroqChatCompletions;

impl Endpoint for GroqChatCompletions {
    const PATH: &'static str = "v1/chat/completions";
    type RequestBody = OpenAICompatibleChatCompletionRequest;
    type ResponseBody = Value;
    type StreamResponseBody = Value;
    type ErrorResponseBody = Value;
}

pub struct GroqConverter {
    model_mapper: ModelMapper,
}

impl GroqConverter {
    #[must_use]
    pub fn new(model_mapper: ModelMapper) -> Self {
        Self { model_mapper }
    }
}

impl
    TryConvert<
        async_openai::types::chat::CreateChatCompletionRequest,
        OpenAICompatibleChatCompletionRequest,
    > for GroqConverter
{
    type Error = MapperError;
    fn try_convert(
        &self,
        mut value: async_openai::types::chat::CreateChatCompletionRequest,
    ) -> Result<OpenAICompatibleChatCompletionRequest, Self::Error> {
        let source_model = ModelId::from_str(&value.model)?;
        let target_model = self.model_mapper.map_model(
            &source_model,
            &InferenceProvider::Named("groq".into()),
        )?;
        tracing::trace!(source_model = ?source_model, target_model = ?target_model, "mapped model");

        value.model = target_model.to_string();

        Ok(OpenAICompatibleChatCompletionRequest {
            provider: InferenceProvider::Named("groq".into()),
            inner: value,
        })
    }
}

impl TryConvert<Value, CreateChatCompletionResponse> for GroqConverter {
    type Error = MapperError;
    fn try_convert(
        &self,
        mut value: Value,
    ) -> Result<CreateChatCompletionResponse, Self::Error> {
        if let Some(obj) = value.as_object_mut()
            && let Some(tier) = obj.get("service_tier")
            && tier == "on_demand"
        {
            obj.insert(
                "service_tier".to_string(),
                serde_json::json!("default"),
            );
        }
        serde_json::from_value(value).map_err(MapperError::SerdeError)
    }
}

impl TryConvertStreamData<Value, CreateChatCompletionStreamResponse>
    for GroqConverter
{
    type Error = MapperError;
    fn try_convert_chunk(
        &self,
        mut value: Value,
    ) -> Result<Option<CreateChatCompletionStreamResponse>, Self::Error> {
        if let Some(obj) = value.as_object_mut()
            && let Some(tier) = obj.get("service_tier")
            && tier == "on_demand"
        {
            obj.insert(
                "service_tier".to_string(),
                serde_json::json!("default"),
            );
        }
        let chunk: CreateChatCompletionStreamResponse =
            serde_json::from_value(value).map_err(MapperError::SerdeError)?;
        Ok(Some(chunk))
    }
}

impl TryConvertError<Value, async_openai::error::WrappedError>
    for GroqConverter
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
