use async_openai::types::chat::{
    CreateChatCompletionRequest, CreateChatCompletionResponse,
    CreateChatCompletionStreamResponse,
};
use http::response::Parts;
use serde_json::{Map, Value};
use web_structured_output::request_requires_json_schema;

use super::{TryConvert, TryConvertStreamData};
use crate::{
    endpoints::openrouter::chat_completions::CreateChatCompletionRequestOpenRouter,
    error::mapper::MapperError,
    middleware::mapper::{
        TryConvertError, model::ModelMapper, openai_chat_response,
        openai_error_from_value, parse_openai_source_model,
    },
    types::provider::InferenceProvider,
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
        let source_model = parse_openai_source_model(&value.model)?;
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
        inject_require_parameters_for_json_schema(&mut value_json);

        Ok(CreateChatCompletionRequestOpenRouter(value_json))
    }
}

fn inject_require_parameters_for_json_schema(value: &mut Value) {
    if !request_requires_json_schema(value) {
        return;
    }

    let Some(obj) = value.as_object_mut() else {
        return;
    };
    let provider = obj
        .entry("provider".to_string())
        .or_insert_with(|| Value::Object(Map::default()));
    if !provider.is_object() {
        *provider = Value::Object(Map::default());
    }
    if let Some(provider_obj) = provider.as_object_mut() {
        provider_obj
            .insert("require_parameters".to_string(), Value::Bool(true));
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
    use crate::{app_state::AppState, middleware::mapper::model::ModelMapper};

    fn chat_request(
        body: Value,
    ) -> async_openai::types::chat::CreateChatCompletionRequest {
        serde_json::from_value(body).expect("valid openai chat request")
    }

    async fn openrouter_converter() -> OpenRouterConverter {
        let app_state = AppState::test_default().await;
        OpenRouterConverter::new(ModelMapper::new(app_state))
    }

    #[tokio::test]
    async fn json_schema_requests_require_openrouter_parameter_support() {
        let converter = openrouter_converter().await;
        let mapped = converter
            .try_convert(chat_request(serde_json::json!({
                "model": "gpt-5-mini",
                "messages": [{"role": "user", "content": "ping"}],
                "response_format": {
                    "type": "json_schema",
                    "json_schema": {
                        "name": "result",
                        "strict": true,
                        "schema": {
                            "type": "object",
                            "properties": {"ok": {"type": "boolean"}},
                            "required": ["ok"],
                            "additionalProperties": false
                        }
                    }
                }
            })))
            .expect("json schema request should map");

        assert_eq!(
            mapped.0.pointer("/provider/require_parameters"),
            Some(&Value::Bool(true))
        );
    }

    #[tokio::test]
    async fn plain_requests_do_not_add_openrouter_provider_routing() {
        let converter = openrouter_converter().await;
        let mapped = converter
            .try_convert(chat_request(serde_json::json!({
                "model": "gpt-5-mini",
                "messages": [{"role": "user", "content": "ping"}]
            })))
            .expect("plain request should map");

        assert!(mapped.0.get("provider").is_none());
    }

    #[test]
    fn preserves_existing_openrouter_provider_options() {
        let mut value = serde_json::json!({
            "model": "openrouter/free",
            "messages": [{"role": "user", "content": "ping"}],
            "provider": {"order": ["OpenAI"]},
            "response_format": {
                "type": "json_schema",
                "json_schema": {
                    "name": "result",
                    "strict": true,
                    "schema": {"type": "object"}
                }
            }
        });

        inject_require_parameters_for_json_schema(&mut value);

        assert_eq!(
            value.pointer("/provider/order/0"),
            Some(&Value::String("OpenAI".into()))
        );
        assert_eq!(
            value.pointer("/provider/require_parameters"),
            Some(&Value::Bool(true))
        );
    }

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
