use std::str::FromStr;

use anthropic_ai_sdk::types::message as anthropic;
use async_openai::types::chat as openai;

use crate::{
    error::mapper::MapperError,
    middleware::mapper::{TryConvert, model::ModelMapper},
    types::{model_id::ModelId, provider::InferenceProvider},
};

pub struct OpenAIConverter {
    pub(crate) model_mapper: ModelMapper,
}

impl OpenAIConverter {
    pub fn new(model_mapper: ModelMapper) -> Self {
        Self { model_mapper }
    }
}

impl
    TryConvert<
        anthropic::CreateMessageParams,
        openai::CreateChatCompletionRequest,
    > for OpenAIConverter
{
    type Error = MapperError;

    #[allow(clippy::too_many_lines)]
    fn try_convert(
        &self,
        value: anthropic::CreateMessageParams,
    ) -> Result<openai::CreateChatCompletionRequest, Self::Error> {
        let source_model = ModelId::from_str(&value.model)?;
        let target_model = self
            .model_mapper
            .map_model(&source_model, &InferenceProvider::OpenAI)?;

        let reasoning_effort = if let Some(thinking) = value.thinking {
            match thinking.type_ {
                anthropic::ThinkingType::Enabled => {
                    #[allow(clippy::cast_precision_loss)]
                    let reasoning_budget = thinking.budget_tokens as f64
                        / f64::from(value.max_tokens);
                    match reasoning_budget {
                        b if b < 0.33 => Some(openai::ReasoningEffort::Low),
                        b if b < 0.66 => Some(openai::ReasoningEffort::Medium),
                        b if b <= 1.0 => Some(openai::ReasoningEffort::High),
                        _ => Some(openai::ReasoningEffort::Medium),
                    }
                }
            }
        } else {
            None
        };

        let stream = value.stream;
        let stream_options = if stream.is_some_and(|s| s) {
            Some(openai::ChatCompletionStreamOptions {
                include_usage: Some(true),
                include_obfuscation: None,
            })
        } else {
            None
        };

        let tool_choice = match value.tool_choice {
            Some(anthropic::ToolChoice::Auto) => {
                Some(openai::ChatCompletionToolChoiceOption::Mode(
                    openai::ToolChoiceOptions::Auto,
                ))
            }
            Some(anthropic::ToolChoice::None) => {
                Some(openai::ChatCompletionToolChoiceOption::Mode(
                    openai::ToolChoiceOptions::None,
                ))
            }
            Some(anthropic::ToolChoice::Any) => {
                Some(openai::ChatCompletionToolChoiceOption::Mode(
                    openai::ToolChoiceOptions::Required,
                ))
            }
            Some(anthropic::ToolChoice::Tool { name }) => {
                Some(openai::ChatCompletionToolChoiceOption::Function(
                    openai::ChatCompletionNamedToolChoice {
                        function: openai::FunctionName { name: name.clone() },
                    },
                ))
            }
            None => None,
        };

        let tools: Option<Vec<openai::ChatCompletionTools>> =
            value.tools.map(|tools| {
                tools
                    .into_iter()
                    .map(|tool| {
                        openai::ChatCompletionTools::Function(
                            openai::ChatCompletionTool {
                                function: openai::FunctionObject {
                                    name: tool.name,
                                    description: tool.description,
                                    parameters: Some(tool.input_schema),
                                    strict: None,
                                },
                            },
                        )
                    })
                    .collect()
            });

        let mut messages = Vec::with_capacity(value.messages.len());
        if let Some(system_prompt) = value.system {
            messages.push(openai::ChatCompletionRequestMessage::Developer(openai::ChatCompletionRequestDeveloperMessage {
                content: openai::ChatCompletionRequestDeveloperMessageContent::Text(system_prompt),
                name: None,
            }));
        }

        for message in value.messages {
            match message.role {
                anthropic::Role::Assistant => {
                    let mapped_content = match message.content {
                        anthropic::MessageContent::Text { content } => Some(openai::ChatCompletionRequestAssistantMessageContent::Text(content)),
                        anthropic::MessageContent::Blocks { content } => {
                            let parts: Vec<_> = content.into_iter().filter_map(|block| {
                                if let anthropic::ContentBlock::Text { text, .. } = block {
                                    Some(openai::ChatCompletionRequestAssistantMessageContentPart::Text(openai::ChatCompletionRequestMessageContentPartText { text }))
                                } else { None }
                            }).collect();
                            if parts.is_empty() { None } else { Some(openai::ChatCompletionRequestAssistantMessageContent::Array(parts)) }
                        }
                    };
                    let assistant_msg =
                        openai::ChatCompletionRequestAssistantMessage {
                            content: mapped_content,
                            tool_calls: None,
                            refusal: None,
                            name: None,
                            audio: None,
                            #[allow(deprecated)]
                            function_call: None,
                        };
                    messages.push(
                        openai::ChatCompletionRequestMessage::Assistant(
                            assistant_msg,
                        ),
                    );
                }
                anthropic::Role::User => {
                    let content = match message.content {
                        anthropic::MessageContent::Text { content } => openai::ChatCompletionRequestUserMessageContent::Text(content),
                        anthropic::MessageContent::Blocks { content } => openai::ChatCompletionRequestUserMessageContent::Array(
                            content.into_iter().filter_map(|block| {
                                match block {
                                    anthropic::ContentBlock::Text { text, .. } => Some(openai::ChatCompletionRequestUserMessageContentPart::Text(openai::ChatCompletionRequestMessageContentPartText { text })),
                                    anthropic::ContentBlock::Image { source } => Some(openai::ChatCompletionRequestUserMessageContentPart::ImageUrl(openai::ChatCompletionRequestMessageContentPartImage {
                                        image_url: openai::ImageUrl { url: source.data, detail: None },
                                    })),
                                    _ => None,
                                }
                            }).collect()
                        ),
                    };
                    messages.push(openai::ChatCompletionRequestMessage::User(
                        openai::ChatCompletionRequestUserMessage {
                            content,
                            name: None,
                        },
                    ));
                }
            }
        }

        let mut metadata = value.metadata;
        let _user = metadata.as_mut().and_then(|m| m.fields.remove("user_id"));

        let mut builder = openai::CreateChatCompletionRequestArgs::default();
        builder
            .messages(messages)
            .model(target_model.to_string())
            .max_completion_tokens(value.max_tokens);

        if let Some(re) = reasoning_effort {
            builder.reasoning_effort(re);
        }
        if let Some(stop_seq) = value.stop_sequences {
            builder.stop(openai::StopConfiguration::StringArray(stop_seq));
        }
        if let Some(s) = stream {
            builder.stream(s);
        }
        if let Some(so) = stream_options {
            builder.stream_options(so);
        }
        if let Some(t) = value.temperature {
            builder.temperature(t);
        }
        if let Some(tp) = value.top_p {
            builder.top_p(tp);
        }
        if let Some(t) = tools {
            builder.tools(t);
        }
        if let Some(tc) = tool_choice {
            builder.tool_choice(tc);
        }
        if let Some(u) = _user {
            builder.prompt_cache_key(u);
        }

        Ok(builder.build().map_err(|_| MapperError::InvalidRequest)?)
    }
}

impl
    TryConvert<
        openai::CreateChatCompletionRequest,
        openai::CreateChatCompletionRequest,
    > for OpenAIConverter
{
    type Error = MapperError;
    fn try_convert(
        &self,
        mut value: openai::CreateChatCompletionRequest,
    ) -> Result<openai::CreateChatCompletionRequest, Self::Error> {
        let source_model = ModelId::from_str(&value.model)?;
        let target_model = self
            .model_mapper
            .map_model(&source_model, &InferenceProvider::OpenAI)?;
        value.model = target_model.to_string();
        Ok(value)
    }
}
