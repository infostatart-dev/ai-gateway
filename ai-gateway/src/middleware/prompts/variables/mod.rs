use crate::error::{api::ApiError, internal::InternalError};
use regex::Regex;

pub mod message;
pub mod process;
pub mod replace;
pub mod schema;
pub mod utils;
pub mod validate;

pub use process::process_prompt_variables;

pub fn get_variable_regex() -> Result<Regex, ApiError> {
    Regex::new(r"\{\{\s*hc\s*:\s*([a-zA-Z_-][a-zA-Z0-9_-]*)\s*:\s*([a-zA-Z_-][a-zA-Z0-9_-]*)\s*\}\}")
        .map_err(|_| ApiError::Internal(InternalError::Internal))
}
