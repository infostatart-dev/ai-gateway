use std::{
    collections::{HashMap, HashSet},
    hash::BuildHasher,
};

use regex::Regex;

use super::replace::replace_variables;
use crate::error::api::ApiError;

pub fn process_message_variables<I, V>(
    msg: &mut serde_json::Value,
    inputs: &HashMap<String, serde_json::Value, I>,
    regex: &Regex,
    validated: &mut HashSet<String, V>,
) -> Result<(), ApiError>
where
    I: BuildHasher,
    V: BuildHasher,
{
    if let Some(content) = msg.get_mut("content") {
        if let Some(s) = content.as_str() {
            let replaced = replace_variables(s, inputs, regex, validated)?;
            *content = serde_json::Value::String(replaced);
        } else if let Some(arr) = content.as_array_mut() {
            for part in arr {
                if let Some(text) =
                    part.get_mut("text").and_then(|t| t.as_str())
                {
                    let replaced =
                        replace_variables(text, inputs, regex, validated)?;
                    part["text"] = serde_json::Value::String(replaced);
                }
            }
        }
    }
    Ok(())
}
