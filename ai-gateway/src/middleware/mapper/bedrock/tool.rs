use async_openai::types::chat as openai;

use crate::endpoints::bedrock::converse::{
    SpecificToolChoice, Tool, ToolChoice, ToolInputSchema, ToolSpecification,
};

#[must_use]
pub fn map_tools(
    tools: Option<Vec<openai::ChatCompletionTools>>,
) -> Option<Vec<Tool>> {
    tools.map(|ts| {
        ts.into_iter()
            .filter_map(|t_enum| {
                if let openai::ChatCompletionTools::Function(t) = t_enum {
                    let parameters = t.function.parameters.clone()?;
                    let json_value = serde_json::from_value(parameters).ok()?;
                    Some(Tool::ToolSpec {
                        tool_spec: ToolSpecification {
                            name: t.function.name.clone(),
                            description: t.function.description.clone(),
                            input_schema: ToolInputSchema::Json {
                                json: json_value,
                            },
                        },
                    })
                } else {
                    None
                }
            })
            .collect()
    })
}

#[must_use]
pub fn map_tool_choice(
    choice: Option<openai::ChatCompletionToolChoiceOption>,
) -> Option<ToolChoice> {
    match choice {
        Some(openai::ChatCompletionToolChoiceOption::Function(tool)) => {
            Some(ToolChoice::Tool {
                tool: SpecificToolChoice {
                    name: tool.function.name,
                },
            })
        }
        Some(openai::ChatCompletionToolChoiceOption::Mode(
            openai::ToolChoiceOptions::Auto,
        )) => Some(ToolChoice::Auto {
            auto: serde_json::json!({}),
        }),
        Some(openai::ChatCompletionToolChoiceOption::Mode(
            openai::ToolChoiceOptions::Required,
        )) => Some(ToolChoice::Any {
            any: serde_json::json!({}),
        }),
        _ => None,
    }
}
