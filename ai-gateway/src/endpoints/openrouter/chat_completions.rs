use async_openai::types::chat::{
    CreateChatCompletionResponse, CreateChatCompletionStreamResponse,
};
use serde::{Deserialize, Serialize};

use crate::{
    endpoints::AiRequest,
    error::mapper::MapperError,
    types::{model_id::ModelId, provider::InferenceProvider},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct ChatCompletions;

impl crate::endpoints::Endpoint for ChatCompletions {
    const PATH: &'static str = "chat/completions";
    type RequestBody = CreateChatCompletionRequestOpenRouter;
    type ResponseBody = CreateChatCompletionResponse;
    type StreamResponseBody = CreateChatCompletionStreamResponse;
    type ErrorResponseBody = serde_json::Value;
}

#[derive(Clone, Serialize, Debug, Deserialize, PartialEq)]
pub struct CreateChatCompletionRequestOpenRouter(pub(crate) serde_json::Value);

impl AiRequest for CreateChatCompletionRequestOpenRouter {
    fn is_stream(&self) -> bool {
        self.0
            .get("stream")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false)
    }

    fn model(&self) -> Result<ModelId, MapperError> {
        let model_str =
            self.0.get("model").and_then(|v| v.as_str()).unwrap_or("");
        ModelId::from_str_and_provider(InferenceProvider::OpenRouter, model_str)
    }
}
