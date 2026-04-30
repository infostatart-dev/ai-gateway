use std::collections::HashSet;
use crate::{error::api::ApiError, types::extensions::PromptContext};
use super::{get_variable_regex, message::process_message_variables, schema::process_prompt_schema};

pub fn process_prompt_variables(mut body: serde_json::Value, prompt_ctx: &PromptContext) -> Result<serde_json::Value, ApiError> {
    let inputs = match &prompt_ctx.inputs { Some(i) => i, None => return Ok(body) };
    let body_obj = match body.as_object_mut() { Some(o) => o, None => return Ok(body) };
    let regex = get_variable_regex()?;

    if let Some(messages) = body_obj.get_mut("messages").and_then(|m| m.as_array_mut()) {
        let mut validated = HashSet::new();
        for msg in messages { process_message_variables(msg, inputs, &regex, &mut validated)?; }
    }

    if let Some(resp_fmt) = body_obj.get_mut("response_format") {
        let processed = process_prompt_schema(resp_fmt.clone(), inputs, &regex)?;
        body_obj.insert("response_format".to_string(), processed);
    }

    if let Some(tools) = body_obj.get_mut("tools") {
        let processed = process_prompt_schema(tools.clone(), inputs, &regex)?;
        body_obj.insert("tools".to_string(), processed);
    }

    Ok(body)
}
