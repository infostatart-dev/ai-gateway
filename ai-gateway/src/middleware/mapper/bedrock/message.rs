use async_openai::types::chat as openai;
use crate::endpoints::bedrock::converse::{Message, ContentBlock, SystemContentBlock, ImageBlock, ImageSource};

pub fn map_messages(messages: Vec<openai::ChatCompletionRequestMessage>) -> (Vec<Message>, Vec<SystemContentBlock>) {
    let mut mapped_messages = Vec::with_capacity(messages.len());
    let mut system_prompts = Vec::new();

    for message in messages {
        match message {
            openai::ChatCompletionRequestMessage::Developer(msg) => {
                system_prompts.push(SystemContentBlock { text: map_developer_content(msg.content) });
            }
            openai::ChatCompletionRequestMessage::System(msg) => {
                system_prompts.push(SystemContentBlock { text: map_system_content(msg.content) });
            }
            openai::ChatCompletionRequestMessage::User(msg) => {
                mapped_messages.push(Message { role: "user".to_string(), content: map_user_content(msg.content) });
            }
            openai::ChatCompletionRequestMessage::Assistant(msg) => {
                if let Some(content) = map_assistant_content(msg.content) {
                    mapped_messages.push(Message { role: "assistant".to_string(), content });
                }
            }
            openai::ChatCompletionRequestMessage::Tool(msg) => {
                mapped_messages.push(Message { role: "user".to_string(), content: map_tool_content(msg) });
            }
            openai::ChatCompletionRequestMessage::Function(_msg) => {
                // Function messages are handled differently or ignored in this mapping
                // For now, matching original logic which was more complex and required tools access
            }
        }
    }
    (mapped_messages, system_prompts)
}

fn map_developer_content(content: openai::ChatCompletionRequestDeveloperMessageContent) -> String {
    match content {
        openai::ChatCompletionRequestDeveloperMessageContent::Text(text) => text,
        openai::ChatCompletionRequestDeveloperMessageContent::Array(array) => {
            array.into_iter().map(|part| {
                match part {
                    openai::ChatCompletionRequestDeveloperMessageContentPart::Text(text_part) => text_part.text,
                }
            }).collect::<Vec<_>>().join("\n")
        }
    }
}

fn map_system_content(content: openai::ChatCompletionRequestSystemMessageContent) -> String {
    match content {
        openai::ChatCompletionRequestSystemMessageContent::Text(text) => text,
        openai::ChatCompletionRequestSystemMessageContent::Array(array) => {
            array.into_iter().map(|part| {
                match part {
                    openai::ChatCompletionRequestSystemMessageContentPart::Text(text_part) => text_part.text,
                }
            }).collect::<Vec<_>>().join("\n")
        }
    }
}

fn map_user_content(content: openai::ChatCompletionRequestUserMessageContent) -> Vec<ContentBlock> {
    match content {
        openai::ChatCompletionRequestUserMessageContent::Text(text) => vec![ContentBlock::Text { text }],
        openai::ChatCompletionRequestUserMessageContent::Array(array) => {
            array.into_iter().filter_map(|part| {
                match part {
                    openai::ChatCompletionRequestUserMessageContentPart::Text(text) => Some(ContentBlock::Text { text: text.text }),
                    openai::ChatCompletionRequestUserMessageContentPart::ImageUrl(image) => {
                        if image.image_url.url.starts_with("http") {
                            None
                        } else {
                            Some(ContentBlock::Image {
                                image: ImageBlock {
                                    format: "png".to_string(),
                                    source: ImageSource { bytes: image.image_url.url },
                                },
                            })
                        }
                    }
                    _ => None,
                }
            }).collect()
        }
    }
}

fn map_assistant_content(content: Option<openai::ChatCompletionRequestAssistantMessageContent>) -> Option<Vec<ContentBlock>> {
    content.map(|c| match c {
        openai::ChatCompletionRequestAssistantMessageContent::Text(text) => vec![ContentBlock::Text { text }],
        openai::ChatCompletionRequestAssistantMessageContent::Array(array) => {
            array.into_iter().map(|part| match part {
                openai::ChatCompletionRequestAssistantMessageContentPart::Text(text) => ContentBlock::Text { text: text.text },
                openai::ChatCompletionRequestAssistantMessageContentPart::Refusal(text) => ContentBlock::Text { text: text.refusal },
            }).collect()
        }
    })
}

fn map_tool_content(msg: openai::ChatCompletionRequestToolMessage) -> Vec<ContentBlock> {
    match msg.content {
        openai::ChatCompletionRequestToolMessageContent::Text(text) => vec![ContentBlock::ToolResult {
            tool_result: crate::endpoints::bedrock::converse::ToolResultBlock {
                tool_use_id: msg.tool_call_id,
                content: vec![crate::endpoints::bedrock::converse::ToolResultContentBlock::Text { text }],
                status: None,
            },
        }],
        openai::ChatCompletionRequestToolMessageContent::Array(array) => array.into_iter().map(|part| match part {
            openai::ChatCompletionRequestToolMessageContentPart::Text(text) => ContentBlock::ToolResult {
                tool_result: crate::endpoints::bedrock::converse::ToolResultBlock {
                    tool_use_id: msg.tool_call_id.clone(),
                    content: vec![crate::endpoints::bedrock::converse::ToolResultContentBlock::Text { text: text.text }],
                    status: None,
                },
            },
        }).collect(),
    }
}
