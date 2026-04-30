use anthropic_ai_sdk::types::message as anthropic;
use async_openai::types::chat as openai;

pub fn map_tools(
    tools: &Option<Vec<openai::ChatCompletionTools>>,
) -> Option<Vec<anthropic::Tool>> {
    tools.as_ref().map(|tools| {
        tools
            .iter()
            .filter_map(|tool_enum| {
                if let openai::ChatCompletionTools::Function(tool) = tool_enum {
                    Some(anthropic::Tool {
                        name: tool.function.name.clone(),
                        description: tool.function.description.clone(),
                        input_schema: tool
                            .function
                            .parameters
                            .clone()
                            .unwrap_or_default(),
                    })
                } else {
                    None
                }
            })
            .collect()
    })
}

pub fn map_tool_choice(
    choice: &Option<openai::ChatCompletionToolChoiceOption>,
) -> Option<anthropic::ToolChoice> {
    match choice {
        Some(openai::ChatCompletionToolChoiceOption::Function(tool)) => {
            Some(anthropic::ToolChoice::Tool {
                name: tool.function.name.clone(),
            })
        }
        Some(openai::ChatCompletionToolChoiceOption::Mode(
            openai::ToolChoiceOptions::Auto,
        )) => Some(anthropic::ToolChoice::Auto),
        Some(openai::ChatCompletionToolChoiceOption::Mode(
            openai::ToolChoiceOptions::Required,
        )) => Some(anthropic::ToolChoice::Any),
        Some(openai::ChatCompletionToolChoiceOption::Mode(
            openai::ToolChoiceOptions::None,
        )) => Some(anthropic::ToolChoice::None),
        _ => None,
    }
}
