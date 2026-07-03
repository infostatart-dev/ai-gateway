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
        model::ModelMapper, openai_chat_response, openai_error_from_value,
        parse_openai_source_model,
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

#[derive(Debug, Clone)]
pub struct ChatGptWebConverter {
    model_mapper: ModelMapper,
}

impl ChatGptWebConverter {
    #[must_use]
    pub fn new(model_mapper: ModelMapper) -> Self {
        Self { model_mapper }
    }
}

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
        let provider = InferenceProvider::Named("chatgpt-web".into());
        let source_model = parse_openai_source_model(&value.model)?;
        let target_model =
            self.model_mapper.map_model(&source_model, &provider)?;
        tracing::trace!(source_model = ?source_model, target_model = ?target_model, "mapped model");
        value.model = target_model.to_string();
        chatgpt_json_schema::inject_json_schema(&mut value);
        Ok(OpenAICompatibleChatCompletionRequest {
            provider,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{app_state::AppState, middleware::mapper::model::ModelMapper};

    fn chat_request(
        model: &str,
    ) -> async_openai::types::chat::CreateChatCompletionRequest {
        serde_json::from_value(serde_json::json!({
            "model": model,
            "messages": [{"role": "user", "content": "ping"}]
        }))
        .expect("valid openai chat request")
    }

    #[tokio::test]
    async fn maps_bare_openai_model_to_chatgpt_web_binding() {
        let app_state = AppState::test_default().await;
        let converter = ChatGptWebConverter::new(ModelMapper::new(app_state));

        let mapped = converter
            .try_convert(chat_request("gpt-5-mini"))
            .expect("bare openai model should map");

        assert_eq!(mapped.inner.model, "gpt-5.5-instant");
    }
}
