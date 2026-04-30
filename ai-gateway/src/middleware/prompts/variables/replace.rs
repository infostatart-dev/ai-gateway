use std::collections::{HashMap, HashSet};
use regex::Regex;
use crate::error::{api::ApiError, invalid_req::InvalidRequestError};
use super::validate::validate_variable_type;

pub fn replace_variables(text: &str, inputs: &HashMap<String, serde_json::Value>, regex: &Regex, validated: &mut HashSet<String>) -> Result<String, ApiError> {
    let mut result = text.to_string();
    for cap in regex.captures_iter(text) {
        let full_match = &cap[0];
        let var_type = &cap[1];
        let var_name = &cap[2];

        if let Some(val) = inputs.get(var_name) {
            if !validated.contains(var_name) {
                validate_variable_type(var_name, var_type, val)?;
                validated.insert(var_name.to_string());
            }
            let str_val = match val {
                serde_json::Value::String(s) => s.clone(),
                _ => val.to_string(),
            };
            result = result.replace(full_match, &str_val);
        } else {
            return Err(ApiError::InvalidRequest(InvalidRequestError::InvalidPromptInputs(format!("Variable '{var_name}' not found in inputs"))));
        }
    }
    Ok(result)
}
