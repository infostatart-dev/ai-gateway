use super::{config, message, tool};
use crate::{
    endpoints::bedrock::converse::{BedrockConverseRequest, ToolConfig},
    error::mapper::MapperError,
    middleware::mapper::{TryConvert, model::ModelMapper},
    types::{model_id::ModelId, provider::InferenceProvider},
};
use async_openai::types::chat as openai;
use std::str::FromStr;

pub struct BedrockConverter {
    model_mapper: ModelMapper,
}

impl BedrockConverter {
    #[must_use]
    pub fn new(model_mapper: ModelMapper) -> Self {
        Self { model_mapper }
    }
}

impl TryConvert<openai::CreateChatCompletionRequest, BedrockConverseRequest>
    for BedrockConverter
{
    type Error = MapperError;
    fn try_convert(
        &self,
        value: openai::CreateChatCompletionRequest,
    ) -> Result<BedrockConverseRequest, Self::Error> {
        let source_model = ModelId::from_str(&value.model)?;
        let target_model = self
            .model_mapper
            .map_model(&source_model, &InferenceProvider::Bedrock)?;

        let (mapped_messages, system_prompts) =
            message::map_messages(value.messages);
        let tools = tool::map_tools(value.tools);
        let tool_choice = tool::map_tool_choice(value.tool_choice);

        let tool_config = tools.map(|ts| ToolConfig {
            tools: ts,
            tool_choice,
        });
        let inference_config = config::map_inference_config(
            value.max_completion_tokens,
            value.temperature,
            value.top_p,
            value.stop,
        );

        Ok(BedrockConverseRequest {
            model_id: Some(target_model.to_string()),
            messages: mapped_messages,
            system: if system_prompts.is_empty() {
                None
            } else {
                Some(system_prompts)
            },
            inference_config,
            tool_config,
        })
    }
}
