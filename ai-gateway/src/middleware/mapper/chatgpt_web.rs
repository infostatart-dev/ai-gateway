use async_openai::types::chat::{
    CreateChatCompletionResponse, CreateChatCompletionStreamResponse,
};
use http::response::Parts;
use serde_json::Value;

use crate::{
    endpoints::{Endpoint, openai::OpenAICompatibleChatCompletionRequest},
    error::mapper::MapperError,
    middleware::mapper::{
        TryConvert, TryConvertError, TryConvertStreamData, chatgpt_json_schema,
        openai_chat_response, openai_error_from_value,
    },
    types::provider::InferenceProvider,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct ChatGptWebChatCompletions;

impl Endpoint for ChatGptWebChatCompletions {
    const PATH: &'static str = "v1/chat/completions";
    type RequestBody = OpenAICompatibleChatCompletionRequest;
    type ResponseBody = Value;
    type StreamResponseBody = Value;
    type ErrorResponseBody = Value;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ChatGptWebConverter;

impl
    TryConvert<
        async_openai::types::chat::CreateChatCompletionRequest,
        OpenAICompatibleChatCompletionRequest,
    > for ChatGptWebConverter
{
    type Error = MapperError;
    fn try_convert(
        &self,
        mut value: async_openai::types::chat::CreateChatCompletionRequest,
    ) -> Result<OpenAICompatibleChatCompletionRequest, Self::Error> {
        chatgpt_json_schema::inject_json_schema(&mut value);
        Ok(OpenAICompatibleChatCompletionRequest {
            provider: InferenceProvider::Named("chatgpt-web".into()),
            inner: value,
        })
    }
}

impl TryConvert<Value, CreateChatCompletionResponse> for ChatGptWebConverter {
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
    for ChatGptWebConverter
{
    type Error = MapperError;
    fn try_convert_chunk(
        &self,
        value: Value,
    ) -> Result<Option<CreateChatCompletionStreamResponse>, Self::Error> {
        serde_json::from_value(value)
            .map(Some)
            .map_err(MapperError::SerdeError)
    }
}

impl TryConvertError<Value, async_openai::error::WrappedError>
    for ChatGptWebConverter
{
    type Error = MapperError;
    fn try_convert_error(
        &self,
        parts: &Parts,
        value: Value,
    ) -> Result<async_openai::error::WrappedError, Self::Error> {
        Ok(openai_error_from_value(parts.status, &value))
    }
}
