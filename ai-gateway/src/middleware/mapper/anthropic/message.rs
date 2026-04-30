use crate::middleware::mapper::mime_from_data_uri;
use anthropic_ai_sdk::types::message as anthropic;
use async_openai::types::chat as openai;

pub fn map_user_content(
    content: openai::ChatCompletionRequestUserMessageContent,
) -> anthropic::MessageContent {
    match content {
        openai::ChatCompletionRequestUserMessageContent::Text(content) => {
            anthropic::MessageContent::Text { content }
        }
        openai::ChatCompletionRequestUserMessageContent::Array(content) => {
            let mapped_content_blocks = content.into_iter().filter_map(|part| {
                match part {
                    openai::ChatCompletionRequestUserMessageContentPart::Text(text) => {
                        Some(anthropic::ContentBlock::Text { text: text.text })
                    }
                    openai::ChatCompletionRequestUserMessageContentPart::ImageUrl(image) => {
                        if image.image_url.url.starts_with("http") {
                            Some(anthropic::ContentBlock::Image {
                                source: anthropic::ImageSource {
                                    type_: "url".to_string(),
                                    media_type: String::new(),
                                    data: image.image_url.url,
                                },
                            })
                        } else {
                            let mime = mime_from_data_uri(&image.image_url.url)?;
                            let (_, b64) = image.image_url.url.split_once(',')?;
                            Some(anthropic::ContentBlock::Image {
                                source: anthropic::ImageSource {
                                    type_: "base64".to_string(),
                                    media_type: mime.mime_type().to_string(),
                                    data: b64.to_string(),
                                },
                            })
                        }
                    }
                    _ => None,
                }
            }).collect();
            anthropic::MessageContent::Blocks {
                content: mapped_content_blocks,
            }
        }
    }
}

pub fn map_assistant_content(
    message: &openai::ChatCompletionRequestAssistantMessage,
) -> Vec<anthropic::ContentBlock> {
    let mut content_blocks = Vec::new();

    match &message.content {
        Some(openai::ChatCompletionRequestAssistantMessageContent::Text(
            content,
        )) => {
            if !content.is_empty() {
                content_blocks.push(anthropic::ContentBlock::Text {
                    text: content.clone(),
                });
            }
        }
        Some(openai::ChatCompletionRequestAssistantMessageContent::Array(
            content,
        )) => {
            for part in content {
                match part {
                    openai::ChatCompletionRequestAssistantMessageContentPart::Text(text) => {
                        content_blocks.push(anthropic::ContentBlock::Text { text: text.text.clone() });
                    }
                    openai::ChatCompletionRequestAssistantMessageContentPart::Refusal(text) => {
                        content_blocks.push(anthropic::ContentBlock::Text { text: text.refusal.clone() });
                    }
                }
            }
        }
        None => {}
    }

    if let Some(tool_calls) = &message.tool_calls {
        for tool_call_enum in tool_calls {
            if let openai::ChatCompletionMessageToolCalls::Function(tool_call) =
                tool_call_enum
            {
                let input = if tool_call.function.arguments.is_empty() {
                    serde_json::Value::Object(serde_json::Map::new())
                } else {
                    serde_json::from_str(&tool_call.function.arguments)
                        .unwrap_or_else(|_| {
                            serde_json::Value::Object(serde_json::Map::new())
                        })
                };

                content_blocks.push(anthropic::ContentBlock::ToolUse {
                    id: tool_call.id.clone(),
                    name: tool_call.function.name.clone(),
                    input,
                });
            }
        }
    }

    content_blocks
}

pub fn map_tool_message(
    message: openai::ChatCompletionRequestToolMessage,
) -> anthropic::MessageContent {
    match message.content {
        openai::ChatCompletionRequestToolMessageContent::Text(content) => {
            let block = anthropic::ContentBlock::ToolResult {
                tool_use_id: message.tool_call_id,
                content,
            };
            anthropic::MessageContent::Blocks {
                content: vec![block],
            }
        }
        openai::ChatCompletionRequestToolMessageContent::Array(content) => {
            let mapped_content_blocks = content.into_iter().map(|part| {
                match part {
                    openai::ChatCompletionRequestToolMessageContentPart::Text(text) => {
                        anthropic::ContentBlock::ToolResult {
                            tool_use_id: message.tool_call_id.clone(),
                            content: text.text
                        }
                    }
                }
            }).collect();
            anthropic::MessageContent::Blocks {
                content: mapped_content_blocks,
            }
        }
    }
}
