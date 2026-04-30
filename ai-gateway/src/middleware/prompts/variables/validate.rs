use crate::error::{api::ApiError, invalid_req::InvalidRequestError};

pub fn validate_variable_type(name: &str, expected: &str, value: &serde_json::Value) -> Result<(), ApiError> {
    let matches = match expected {
        "text" | "string" => value.is_string(),
        "number" => value.is_number(),
        "boolean" | "bool" => value.is_boolean(),
        "object" => value.is_object(),
        "array" => value.is_array(),
        _ => true,
    };
    if !matches {
        return Err(ApiError::InvalidRequest(InvalidRequestError::InvalidPromptInputs(
            format!("Variable '{name}' has type '{expected}' but received value: {value}")
        )));
    }
    Ok(())
}
