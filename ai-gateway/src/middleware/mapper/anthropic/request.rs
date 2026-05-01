use std::{collections::HashMap, str::FromStr};

use anthropic_ai_sdk::types::message as anthropic;
use async_openai::types::chat as openai;

use super::{message, tool};
use crate::{
    endpoints::openai::chat_completions::system_prompt,
    error::mapper::MapperError,
    middleware::mapper::{DEFAULT_MAX_TOKENS, TryConvert, model::ModelMapper},
    types::{
        model_id::{ModelId, Version},
        provider::InferenceProvider,
    },
};

pub struct AnthropicConverter {
    pub(crate) model_mapper: ModelMapper,
}

impl AnthropicConverter {
    pub fn new(model_mapper: ModelMapper) -> Self {
        Self { model_mapper }
    }
}

impl
    TryConvert<
        openai::CreateChatCompletionRequest,
        anthropic::CreateMessageParams,
    > for AnthropicConverter
{
    type Error = MapperError;

    fn try_convert(
        &self,
        value: openai::CreateChatCompletionRequest,
    ) -> Result<anthropic::CreateMessageParams, Self::Error> {
        let source_model = ModelId::from_str(&value.model)?;
        let mut target_model = self
            .model_mapper
            .map_model(&source_model, &InferenceProvider::Anthropic)?;

        if let ModelId::ModelIdWithVersion {
            provider: InferenceProvider::Anthropic,
            id: model,
        } = &mut target_model
        {
            if model.model.contains("claude-3") {
                model.version = Version::Latest;
            } else {
                model.version = Version::ImplicitLatest;
            }
        }

        let system_prompt = system_prompt(&value);
        let max_tokens = value
            .max_completion_tokens
            .or(
                #[allow(deprecated)]
                value.max_tokens,
            )
            .unwrap_or(DEFAULT_MAX_TOKENS);
        let temperature = value.temperature;
        let stop_sequences = match value.stop {
            Some(openai::StopConfiguration::String(stop)) => Some(vec![stop]),
            Some(openai::StopConfiguration::StringArray(stops)) => Some(stops),
            _ => None,
        };
        let stream = value.stream;
        let top_p = value.top_p;
        let tools = tool::map_tools(&value.tools);
        let user = value.prompt_cache_key.or(value.safety_identifier).or(
            #[allow(deprecated)]
            value.user,
        );
        let metadata = user.map(|u| anthropic::Metadata {
            fields: HashMap::from([("user_id".to_string(), u)]),
        });
        let tool_choice = tool::map_tool_choice(&value.tool_choice);

        let mut mapped_messages = Vec::with_capacity(value.messages.len());
        for message in value.messages {
            match message {
                openai::ChatCompletionRequestMessage::Developer(_)
                | openai::ChatCompletionRequestMessage::System(_) => {}
                openai::ChatCompletionRequestMessage::User(msg) => {
                    mapped_messages.push(anthropic::Message {
                        role: anthropic::Role::User,
                        content: message::map_user_content(msg.content),
                    });
                }
                openai::ChatCompletionRequestMessage::Assistant(msg) => {
                    let content_blocks = message::map_assistant_content(&msg);
                    if !content_blocks.is_empty() {
                        mapped_messages.push(anthropic::Message {
                            role: anthropic::Role::Assistant,
                            content: anthropic::MessageContent::Blocks {
                                content: content_blocks,
                            },
                        });
                    }
                }
                openai::ChatCompletionRequestMessage::Tool(msg) => {
                    mapped_messages.push(anthropic::Message {
                        role: anthropic::Role::User,
                        content: message::map_tool_message(msg),
                    });
                }
                openai::ChatCompletionRequestMessage::Function(_) => {
                    // Function messages are handled via tools mapping in
                    // Anthropic
                }
            }
        }

        Ok(anthropic::CreateMessageParams {
            max_tokens,
            messages: mapped_messages,
            model: target_model.to_string(),
            system: system_prompt,
            temperature,
            stop_sequences,
            stream,
            top_k: None,
            top_p,
            tools,
            tool_choice,
            metadata,
            thinking: None,
        })
    }
}

// Passthrough implementation
impl TryConvert<anthropic::CreateMessageParams, anthropic::CreateMessageParams>
    for AnthropicConverter
{
    type Error = MapperError;
    fn try_convert(
        &self,
        mut value: anthropic::CreateMessageParams,
    ) -> Result<anthropic::CreateMessageParams, Self::Error> {
        let source_model = ModelId::from_str(&value.model)?;
        let target_model = self
            .model_mapper
            .map_model(&source_model, &InferenceProvider::Anthropic)?;
        value.model = target_model.to_string();
        Ok(value)
    }
}
