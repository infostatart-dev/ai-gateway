use regex::Regex;

use crate::error::{api::ApiError, internal::InternalError};

pub fn is_whole_variable_match(text: &str, regex: &Regex) -> bool {
    regex
        .find(text)
        .map(|m| m.as_str() == text)
        .unwrap_or(false)
}

pub fn get_variable_name_from_string(
    text: &str,
    regex: &Regex,
) -> Result<String, ApiError> {
    regex
        .captures(text)
        .and_then(|c| c.get(2))
        .map(|m| m.as_str().to_string())
        .ok_or_else(|| ApiError::Internal(InternalError::Internal))
}
