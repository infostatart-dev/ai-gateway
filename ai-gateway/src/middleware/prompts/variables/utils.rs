use regex::Regex;

use crate::error::{api::ApiError, internal::InternalError};

#[must_use]
pub fn is_whole_variable_match(text: &str, regex: &Regex) -> bool {
    regex.find(text).is_some_and(|m| m.as_str() == text)
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
