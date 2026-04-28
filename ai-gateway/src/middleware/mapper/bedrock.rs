use std::str::FromStr;

use async_openai::types::{
    CreateChatCompletionResponse, CreateChatCompletionStreamResponse,
};
use http::response::Parts;
use uuid::Uuid;

use super::{
    MapperError, TryConvert, TryConvertStreamData, model::ModelMapper,
};
use crate::{
    middleware::mapper::{DEFAULT_MAX_TOKENS, TryConvertError},
    types::{model_id::ModelId, provider::InferenceProvider},
    endpoints::bedrock::converse::{
        BedrockConverseRequest, BedrockConverseResponse, BedrockConverseStreamOutput, ContentBlock, ImageBlock, ImageSource,
        InferenceConfig, Message, SpecificToolChoice, SystemContentBlock, Tool, ToolChoice,
        ToolConfig, ToolInputSchema, ToolResultBlock, ToolResultContentBlock, ToolSpecification,
        ToolUseBlock,
    },
};

pub struct BedrockConverter {
    model_mapper: ModelMapper,
}

impl BedrockConverter {
    #[must_use]
    pub fn new(model_mapper: ModelMapper) -> Self {
        Self { model_mapper }
    }
}

impl
    TryConvert<
        async_openai::types::CreateChatCompletionRequest,
        BedrockConverseRequest,
    > for BedrockConverter
{
    type Error = MapperError;
    #[allow(clippy::too_many_lines)]
    fn try_convert(
        &self,
        value: async_openai::types::CreateChatCompletionRequest,
    ) -> Result<
        BedrockConverseRequest,
        Self::Error,
    > {
        use async_openai::types as openai;
        let source_model = ModelId::from_str(&value.model)?;

        let target_model = self
            .model_mapper
            .map_model(&source_model, &InferenceProvider::Bedrock)?;

        tracing::trace!(source_model = ?source_model, target_model = ?target_model, "mapped model");

        let max_tokens =
            value.max_completion_tokens.unwrap_or(DEFAULT_MAX_TOKENS);
        let stop_sequences = match value.stop {
            Some(openai::Stop::String(stop)) => Some(vec![stop]),
            Some(openai::Stop::StringArray(stops)) => Some(stops),
            None => None,
        };
        let temperature = value.temperature;
        let top_p = value.top_p;

        let tool_choice = match value.tool_choice {
            Some(openai::ChatCompletionToolChoiceOption::Named(tool)) => {
                Some(ToolChoice::Tool {
                    tool: SpecificToolChoice {
                        name: tool.function.name,
                    },
                })
            }
            Some(openai::ChatCompletionToolChoiceOption::Auto) => {
                Some(ToolChoice::Auto { auto: serde_json::json!({}) })
            }
            Some(openai::ChatCompletionToolChoiceOption::Required) => {
                Some(ToolChoice::Any { any: serde_json::json!({}) })
            }
            Some(openai::ChatCompletionToolChoiceOption::None) | None => None,
        };

        let tools = if let Some(tools) = value.tools {
            let mapped_tools: Vec<_> = tools
                .into_iter()
                .filter_map(|tool| {
                    let parameters = tool.function.parameters?;
                    let json_value = serde_json::from_value(parameters).ok()?;
                    Some(Tool::ToolSpec {
                        tool_spec: ToolSpecification {
                            name: tool.function.name,
                            description: tool.function.description,
                            input_schema: ToolInputSchema::Json { json: json_value },
                        },
                    })
                })
                .collect();
            Some(mapped_tools)
        } else {
            None
        };

        let mut mapped_messages = Vec::with_capacity(value.messages.len());
        let mut system_prompts = Vec::new();

        for message in value.messages {
            match message {
                openai::ChatCompletionRequestMessage::Developer(message) => {
                    let text = match message.content {
                        openai::ChatCompletionRequestDeveloperMessageContent::Text(text) => text,
                        openai::ChatCompletionRequestDeveloperMessageContent::Array(array) => {
                            array.into_iter().map(|part| part.text).collect::<Vec<_>>().join("\n")
                        }
                    };
                    system_prompts.push(SystemContentBlock { text });
                }
                openai::ChatCompletionRequestMessage::System(message) => {
                    let text = match message.content {
                        openai::ChatCompletionRequestSystemMessageContent::Text(text) => text,
                        openai::ChatCompletionRequestSystemMessageContent::Array(array) => {
                            array.into_iter().map(|part| {
                                match part {
                                    openai::ChatCompletionRequestSystemMessageContentPart::Text(text) => text.text,
                                }
                            }).collect::<Vec<_>>().join("\n")
                        }
                    };
                    system_prompts.push(SystemContentBlock { text });
                }
                openai::ChatCompletionRequestMessage::User(message) => {
                    let mapped_content: Vec<ContentBlock> = match message.content {
                        openai::ChatCompletionRequestUserMessageContent::Text(content) => {
                            vec![ContentBlock::Text { text: content }]
                        }
                        openai::ChatCompletionRequestUserMessageContent::Array(content) => {
                            content.into_iter().filter_map(|part| {
                                match part {
                                    openai::ChatCompletionRequestUserMessageContentPart::Text(text) => {
                                        Some(ContentBlock::Text { text: text.text })
                                    }
                                    openai::ChatCompletionRequestUserMessageContentPart::ImageUrl(image) => {
                                        if image.image_url.url.starts_with("http") {
                                            None // Bedrock doesn't support direct HTTP URLs for images yet
                                        } else {
                                            Some(ContentBlock::Image {
                                                image: ImageBlock {
                                                    format: "png".to_string(), // Defaulting, could be inferred
                                                    source: ImageSource {
                                                        bytes: image.image_url.url,
                                                    },
                                                },
                                            })
                                        }
                                    }
                                    openai::ChatCompletionRequestUserMessageContentPart::InputAudio(_audio) => {
                                        None
                                    }
                                }
                            }).collect()
                        }
                    };
                    mapped_messages.push(Message {
                        role: "user".to_string(),
                        content: mapped_content,
                    });
                }
                openai::ChatCompletionRequestMessage::Assistant(message) => {
                    let mapped_content = match message.content {
                        Some(openai::ChatCompletionRequestAssistantMessageContent::Text(content)) => {
                            vec![ContentBlock::Text { text: content }]
                        }
                        Some(openai::ChatCompletionRequestAssistantMessageContent::Array(content)) => {
                            content.into_iter().map(|part| {
                                match part {
                                    openai::ChatCompletionRequestAssistantMessageContentPart::Text(text) => {
                                        ContentBlock::Text { text: text.text }
                                    }
                                    openai::ChatCompletionRequestAssistantMessageContentPart::Refusal(text) => {
                                        ContentBlock::Text { text: text.refusal }
                                    }
                                }
                            }).collect()
                        }
                        None => continue,
                    };
                    mapped_messages.push(Message {
                        role: "assistant".to_string(),
                        content: mapped_content,
                    });
                }
                openai::ChatCompletionRequestMessage::Tool(message) => {
                    let mapped_content = match message.content {
                        openai::ChatCompletionRequestToolMessageContent::Text(text) => {
                            vec![ContentBlock::ToolResult {
                                tool_result: ToolResultBlock {
                                    tool_use_id: message.tool_call_id.clone(),
                                    content: vec![ToolResultContentBlock::Text { text }],
                                    status: None,
                                },
                            }]
                        }
                        openai::ChatCompletionRequestToolMessageContent::Array(content) => {
                            content.into_iter().map(|part| {
                                match part {
                                    openai::ChatCompletionRequestToolMessageContentPart::Text(text) => {
                                        ContentBlock::ToolResult {
                                            tool_result: ToolResultBlock {
                                                tool_use_id: message.tool_call_id.clone(),
                                                content: vec![ToolResultContentBlock::Text { text: text.text }],
                                                status: None,
                                            },
                                        }
                                    }
                                }
                            }).collect()
                        }
                    };
                    mapped_messages.push(Message {
                        role: "user".to_string(), // Tool results are submitted as user messages in Bedrock
                        content: mapped_content,
                    });
                }
                openai::ChatCompletionRequestMessage::Function(message) => {
                    let tools_ref = tools.as_ref();
                    let Some(tool) = tools_ref.and_then(|tools| {
                        tools.iter().find_map(|tool| {
                            let Tool::ToolSpec { tool_spec } = tool;
                            if tool_spec.name == message.name {
                                Some(tool.clone())
                            } else {
                                None
                            }
                        })
                    }) else {
                        continue;
                    };

                    let Tool::ToolSpec { tool_spec } = tool;
                    let input = match tool_spec.input_schema {
                        ToolInputSchema::Json { json } => json,
                    };

                    let mapped_content = vec![ContentBlock::ToolUse {
                        tool_use: ToolUseBlock {
                            tool_use_id: message.name.clone(), // Using name as ID fallback
                            name: message.name.clone(),
                            input,
                        },
                    }];

                    mapped_messages.push(Message {
                        role: "assistant".to_string(),
                        content: mapped_content,
                    });
                }
            }
        }

        let tool_config = tools.map(|tools| ToolConfig {
            tools,
            tool_choice,
        });

        #[allow(clippy::cast_possible_wrap)]
        let inference_config = Some(InferenceConfig {
            top_p,
            temperature,
            max_tokens: Some(i32::try_from(max_tokens).unwrap_or(DEFAULT_MAX_TOKENS as i32)),
            stop_sequences,
        });

        let request = BedrockConverseRequest {
            model_id: Some(target_model.to_string()),
            messages: mapped_messages,
            system: if system_prompts.is_empty() { None } else { Some(system_prompts) },
            inference_config,
            tool_config,
        };

        Ok(request)
    }
}

impl
    TryConvert<
        BedrockConverseResponse,
        CreateChatCompletionResponse,
    > for BedrockConverter
{
    type Error = MapperError;

    #[allow(clippy::too_many_lines, clippy::cast_possible_wrap)]
    fn try_convert(
        &self,
        value: BedrockConverseResponse,
    ) -> std::result::Result<CreateChatCompletionResponse, Self::Error> {
        use async_openai::types as openai;

        // Parse fields dynamically from the raw JSON payload
        let payload = &value.payload;
        let created = 0;
        let model = payload.pointer("/trace/promptRouter/invokedModelId")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();

        let default_usage = openai::CompletionUsage {
            prompt_tokens: 0,
            completion_tokens: 0,
            total_tokens: 0,
            prompt_tokens_details: None,
            completion_tokens_details: None,
        };

        let usage = payload.get("usage").map_or(default_usage, |usage| {
            let input_tokens = u32::try_from(usage.get("inputTokens").and_then(serde_json::Value::as_u64).unwrap_or(0)).unwrap_or(0);
            let output_tokens = u32::try_from(usage.get("outputTokens").and_then(serde_json::Value::as_u64).unwrap_or(0)).unwrap_or(0);
            let total_tokens = u32::try_from(usage.get("totalTokens").and_then(serde_json::Value::as_u64).unwrap_or(0)).unwrap_or(0);
            
            openai::CompletionUsage {
                prompt_tokens: input_tokens,
                completion_tokens: output_tokens,
                total_tokens,
                prompt_tokens_details: None,
                completion_tokens_details: None,
            }
        });

        let mut tool_calls = Vec::new();
        let mut content = None;
        
        if let Some(contents) = payload.pointer("/output/message/content").and_then(serde_json::Value::as_array) {
            for block in contents {
                if let Some(text) = block.get("text").and_then(serde_json::Value::as_str) {
                    content = Some(text.to_string());
                } else if let Some(tool_use) = block.get("toolUse") {
                    let id = tool_use.get("toolUseId").and_then(serde_json::Value::as_str).unwrap_or_default().to_string();
                    let name = tool_use.get("name").and_then(serde_json::Value::as_str).unwrap_or_default().to_string();
                    let arguments = tool_use.get("input").map(std::string::ToString::to_string).unwrap_or_default();
                    
                    tool_calls.push(openai::ChatCompletionMessageToolCall {
                            id,
                            r#type: openai::ChatCompletionToolType::Function,
                            function: openai::FunctionCall {
                                name,
                                arguments,
                            },
                        });
                }
            }
        }

        let tool_calls = if tool_calls.is_empty() {
            None
        } else {
            Some(tool_calls)
        };

        #[allow(deprecated)]
        let message = openai::ChatCompletionResponseMessage {
            content,
            refusal: None,
            tool_calls,
            role: openai::Role::Assistant,
            function_call: None,
            audio: None,
        };

        let choice = openai::ChatChoice {
            index: 0,
            message,
            finish_reason: None,
            logprobs: None,
        };

        let response = openai::CreateChatCompletionResponse {
            choices: vec![choice],
            id: String::from(Uuid::new_v4()),
            created,
            model,
            object: crate::middleware::mapper::anthropic::OPENAI_CHAT_COMPLETION_OBJECT.to_string(),
            usage: Some(usage),
            service_tier: None,
            system_fingerprint: None,
        };
        
        Ok(response)
    }
}

impl
    TryConvertStreamData<
        BedrockConverseStreamOutput,
        CreateChatCompletionStreamResponse,
    > for BedrockConverter
{
    type Error = MapperError;

    #[allow(clippy::too_many_lines)]
    fn try_convert_chunk(
        &self,
        value: BedrockConverseStreamOutput,
    ) -> Result<
        std::option::Option<CreateChatCompletionStreamResponse>,
        Self::Error,
    > {
        use async_openai::types as openai;
        
        let payload = &value.payload;
        let mut choices = Vec::new();
        
        // This parses the JSON chunks as they arrive from the raw Bedrock stream.
        // It provides a reasonable industrial mapping without relying on the AWS SDK's generated types.

        if let Some(message_start) = payload.get("messageStart") {
            let role = match message_start.get("role").and_then(|r| r.as_str()) {
                Some("assistant") => openai::Role::Assistant,
                Some("user") => openai::Role::User,
                _ => openai::Role::System,
            };
            
            choices.push(openai::ChatChoiceStream {
                index: 0,
                delta: openai::ChatCompletionStreamResponseDelta {
                    role: Some(role),
                    content: None,
                    tool_calls: None,
                    refusal: None,
                    #[allow(deprecated)]
                    function_call: None,
                },
                finish_reason: None,
                logprobs: None,
            });
        }
        
        if let Some(tool_use) = payload.pointer("/contentBlockStart/start/toolUse") {
            let id = tool_use.get("toolUseId").and_then(serde_json::Value::as_str).unwrap_or_default().to_string();
            let name = tool_use.get("name").and_then(serde_json::Value::as_str).unwrap_or_default().to_string();
            let index = u32::try_from(payload.pointer("/contentBlockStart/contentBlockIndex").and_then(serde_json::Value::as_u64).unwrap_or(0)).unwrap_or(0);
                
                let tool_call_chunk = openai::ChatCompletionMessageToolCallChunk {
                    index,
                    id: Some(id),
                    r#type: Some(openai::ChatCompletionToolType::Function),
                    function: Some(openai::FunctionCallStream {
                        name: Some(name),
                        arguments: Some(String::new()),
                    }),
                };
                
                choices.push(openai::ChatChoiceStream {
                    index: 0,
                    delta: openai::ChatCompletionStreamResponseDelta {
                        role: None,
                        content: None,
                        tool_calls: Some(vec![tool_call_chunk]),
                        refusal: None,
                        #[allow(deprecated)]
                        function_call: None,
                    },
                    finish_reason: None,
                    logprobs: None,
                });
        }
        
        if let Some(content_block_delta) = payload.get("contentBlockDelta") {
            let index = u32::try_from(content_block_delta.get("contentBlockIndex").and_then(serde_json::Value::as_u64).unwrap_or(0)).unwrap_or(0);
            
            if let Some(text) = content_block_delta.pointer("/delta/text") {
                let text_val = text.as_str().unwrap_or_default().to_string();
                choices.push(openai::ChatChoiceStream {
                    index,
                    delta: openai::ChatCompletionStreamResponseDelta {
                        role: None,
                        content: Some(text_val),
                        tool_calls: None,
                        refusal: None,
                        #[allow(deprecated)]
                        function_call: None,
                    },
                    finish_reason: None,
                    logprobs: None,
                });
            } else if let Some(tool_use) = content_block_delta.pointer("/delta/toolUse") {
                let input = tool_use.get("input").and_then(|v| v.as_str()).unwrap_or_default().to_string();
                let tool_call_chunk = openai::ChatCompletionMessageToolCallChunk {
                    index,
                    id: None,
                    r#type: Some(openai::ChatCompletionToolType::Function),
                    function: Some(openai::FunctionCallStream {
                        name: None,
                        arguments: Some(input),
                    }),
                };
                
                choices.push(openai::ChatChoiceStream {
                    index: 0,
                    delta: openai::ChatCompletionStreamResponseDelta {
                        role: None,
                        content: None,
                        tool_calls: Some(vec![tool_call_chunk]),
                        refusal: None,
                        #[allow(deprecated)]
                        function_call: None,
                    },
                    finish_reason: None,
                    logprobs: None,
                });
            }
        }

        let mut usage = None;
        if let Some(u) = payload.pointer("/metadata/usage") {
            let input_tokens = u32::try_from(u.get("inputTokens").and_then(serde_json::Value::as_u64).unwrap_or(0)).unwrap_or(0);
            let output_tokens = u32::try_from(u.get("outputTokens").and_then(serde_json::Value::as_u64).unwrap_or(0)).unwrap_or(0);
            let total_tokens = u32::try_from(u.get("totalTokens").and_then(serde_json::Value::as_u64).unwrap_or(0)).unwrap_or(0);
            
            usage = Some(openai::CompletionUsage {
                prompt_tokens: input_tokens,
                completion_tokens: output_tokens,
                total_tokens,
                prompt_tokens_details: None,
                completion_tokens_details: None,
            });
        }

        if choices.is_empty() && usage.is_none() {
            // Ignore events like MessageStop that don't add useful data for the OpenAI response format directly
            return Ok(None);
        }

        Ok(Some(CreateChatCompletionStreamResponse {
            id: String::from(Uuid::new_v4()),
            choices,
            created: 0,
            model: "bedrock-stream".to_string(),
            object: "chat.completion.chunk".to_string(),
            system_fingerprint: None,
            service_tier: None,
            usage,
        }))
    }
}

impl
    TryConvertError<
        crate::endpoints::bedrock::converse::ConverseError,
        async_openai::error::WrappedError,
    > for BedrockConverter
{
    type Error = MapperError;

    fn try_convert_error(
        &self,
        resp_parts: &Parts,
        _value: crate::endpoints::bedrock::converse::ConverseError,
    ) -> Result<async_openai::error::WrappedError, Self::Error> {
        Ok(super::openai_error_from_status(resp_parts.status, None))
    }
}
