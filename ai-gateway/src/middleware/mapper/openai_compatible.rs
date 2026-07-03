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
        openai_error_from_value, parse_openai_source_model,
    },
    types::provider::InferenceProvider,
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
        let source_model = parse_openai_source_model(&value.model)?;
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
        if is_longcat_provider(&self.provider) {
            promote_longcat_reasoning_content(&mut value);
        }
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
        if is_longcat_provider(&self.provider) {
            promote_longcat_reasoning_content(&mut value);
        }
        openai_chat_response::normalize_stream_chunk(&mut value);
        let chunk: CreateChatCompletionStreamResponse =
            serde_json::from_value(value).map_err(MapperError::SerdeError)?;
        Ok(Some(chunk))
    }
}

fn is_longcat_provider(provider: &InferenceProvider) -> bool {
    matches!(provider, InferenceProvider::Named(name) if name == "longcat")
}

fn promote_longcat_reasoning_content(value: &mut Value) {
    let Some(choices) = value.get_mut("choices").and_then(Value::as_array_mut)
    else {
        return;
    };
    for choice in choices {
        if let Some(message) = choice.get_mut("message") {
            promote_message_reasoning_content(message);
        }
        if let Some(delta) = choice.get_mut("delta") {
            promote_message_reasoning_content(delta);
        }
    }
}

fn promote_message_reasoning_content(message: &mut Value) {
    let Some(obj) = message.as_object_mut() else {
        return;
    };
    let Some(reasoning) = obj.get("reasoning_content").cloned() else {
        return;
    };
    if !reasoning.is_string() || !content_missing_or_empty(obj.get("content")) {
        return;
    }
    obj.insert("content".to_string(), reasoning);
}

fn content_missing_or_empty(content: Option<&Value>) -> bool {
    match content {
        None | Some(Value::Null) => true,
        Some(Value::String(text)) => text.is_empty(),
        Some(_) => false,
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
    use crate::{
        app_state::AppState, middleware::mapper::openai_chat_response,
    };

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
    async fn maps_bare_openai_model_for_named_compatible_provider() {
        let app_state = AppState::test_default().await;
        let converter = OpenAICompatibleConverter::new(
            InferenceProvider::Named("longcat".into()),
            ModelMapper::new(app_state),
        );

        let mapped = converter
            .try_convert(chat_request("gpt-5-mini"))
            .expect("bare openai model should map");

        assert_eq!(mapped.inner.model, "LongCat-2.0");
    }

    #[test]
    fn longcat_promotes_missing_content_from_reasoning_content() {
        let mut value = serde_json::json!({
            "choices": [{
                "message": {
                    "role": "assistant",
                    "reasoning_content": "{\"ok\":true}"
                },
                "finish_reason": "stop"
            }]
        });

        promote_longcat_reasoning_content(&mut value);

        assert_eq!(value["choices"][0]["message"]["content"], "{\"ok\":true}");
    }

    #[test]
    fn longcat_keeps_existing_content_over_reasoning_content() {
        let mut value = serde_json::json!({
            "choices": [{
                "message": {
                    "role": "assistant",
                    "content": "{\"ok\":true}",
                    "reasoning_content": "hidden"
                },
                "finish_reason": "stop"
            }]
        });

        promote_longcat_reasoning_content(&mut value);

        assert_eq!(value["choices"][0]["message"]["content"], "{\"ok\":true}");
    }

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
