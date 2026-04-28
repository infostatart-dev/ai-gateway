use serde::{Deserialize, Serialize};

use crate::{
    endpoints::{AiRequest, Endpoint},
    error::mapper::MapperError,
    types::{model_id::ModelId, provider::InferenceProvider},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Converse;

// -----------------------------------------------------------------------------
// Anti-Corruption Layer (ACL) for AWS Bedrock
// -----------------------------------------------------------------------------
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BedrockConverseRequest {
    #[serde(skip_serializing)] 
    pub model_id: Option<String>,
    pub messages: Vec<Message>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system: Option<Vec<SystemContentBlock>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inference_config: Option<InferenceConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_config: Option<ToolConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Message {
    pub role: String, // "user" or "assistant"
    pub content: Vec<ContentBlock>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemContentBlock {
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", untagged)]
pub enum ContentBlock {
    Text { text: String },
    Image { image: ImageBlock },
    ToolUse { tool_use: ToolUseBlock },
    ToolResult { tool_result: ToolResultBlock },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageBlock {
    pub format: String,
    pub source: ImageSource,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageSource {
    pub bytes: String, // Base64 encoded string
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolUseBlock {
    pub tool_use_id: String,
    pub name: String,
    pub input: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolResultBlock {
    pub tool_use_id: String,
    pub content: Vec<ToolResultContentBlock>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", untagged)]
pub enum ToolResultContentBlock {
    Text { text: String },
    Image { image: ImageBlock },
    Json { json: serde_json::Value },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InferenceConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_sequences: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolConfig {
    pub tools: Vec<Tool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<ToolChoice>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", untagged)]
pub enum ToolChoice {
    Auto { auto: serde_json::Value },
    Any { any: serde_json::Value },
    Tool { tool: SpecificToolChoice },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpecificToolChoice {
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", untagged)]
pub enum Tool {
    ToolSpec { tool_spec: ToolSpecification },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolSpecification {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub input_schema: ToolInputSchema,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", untagged)]
pub enum ToolInputSchema {
    Json { json: serde_json::Value },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BedrockConverseResponse {
    #[serde(flatten)]
    pub payload: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BedrockConverseStreamOutput {
    #[serde(flatten)]
    pub payload: serde_json::Value,
}

impl Endpoint for Converse {
    const PATH: &'static str = "model/{model_id}/converse";
    type RequestBody = BedrockConverseRequest;
    type ResponseBody = BedrockConverseResponse;
    type StreamResponseBody = BedrockConverseStreamOutput;
    type ErrorResponseBody = ConverseError;
}

impl AiRequest for BedrockConverseRequest {
    fn is_stream(&self) -> bool {
        false
    }

    fn model(&self) -> Result<ModelId, MapperError> {
        let model =
            self.model_id.as_ref().ok_or(MapperError::InvalidRequest)?;
        ModelId::from_str_and_provider(InferenceProvider::Bedrock, model)
    }
}

// The AWS SDK does not document the error format so instead we use a unit
// struct and simply rely on the http status codes to map to the OpenAI error.
#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
pub struct ConverseError;
