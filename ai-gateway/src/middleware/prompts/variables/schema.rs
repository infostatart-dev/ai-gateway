use super::{
    replace::replace_variables,
    utils::{get_variable_name_from_string, is_whole_variable_match},
};
use crate::error::{api::ApiError, invalid_req::InvalidRequestError};
use regex::Regex;
use std::collections::{HashMap, HashSet};

pub fn process_prompt_schema(
    value: serde_json::Value,
    inputs: &HashMap<String, serde_json::Value>,
    regex: &Regex,
) -> Result<serde_json::Value, ApiError> {
    match value {
        serde_json::Value::String(s) => {
            if is_whole_variable_match(&s, regex) {
                let name = get_variable_name_from_string(&s, regex)?;
                if let Some(val) = inputs.get(&name) {
                    return Ok(val.clone());
                }
            }
            Ok(serde_json::Value::String(replace_variables(
                &s,
                inputs,
                regex,
                &mut HashSet::new(),
            )?))
        }
        serde_json::Value::Array(arr) => {
            let mut processed = Vec::new();
            for item in arr {
                processed.push(process_prompt_schema(item, inputs, regex)?);
            }
            Ok(serde_json::Value::Array(processed))
        }
        serde_json::Value::Object(obj) => {
            let mut processed = serde_json::Map::new();
            for (key, val) in obj {
                let p_key = if is_whole_variable_match(&key, regex) {
                    let name = get_variable_name_from_string(&key, regex)?;
                    if let Some(val) = inputs.get(&name) {
                        val.as_str().map(|s| s.to_string()).ok_or_else(|| ApiError::InvalidRequest(InvalidRequestError::InvalidPromptInputs(format!("Variable '{name}' in object schema key must be a string, got: {val}"))))?
                    } else {
                        key
                    }
                } else {
                    replace_variables(&key, inputs, regex, &mut HashSet::new())?
                };
                processed
                    .insert(p_key, process_prompt_schema(val, inputs, regex)?);
            }
            Ok(serde_json::Value::Object(processed))
        }
        _ => Ok(value),
    }
}
