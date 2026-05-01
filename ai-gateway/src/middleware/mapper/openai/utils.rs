use http::StatusCode;

pub(crate) const SERVER_ERROR_TYPE: &str = "server_error";
pub(crate) const INVALID_REQUEST_ERROR_TYPE: &str = "invalid_request_error";

#[must_use]
pub fn get_error_type(status_code: StatusCode) -> String {
    if status_code == StatusCode::TOO_MANY_REQUESTS {
        "tokens".to_string()
    } else if status_code.is_client_error() {
        INVALID_REQUEST_ERROR_TYPE.to_string()
    } else {
        SERVER_ERROR_TYPE.to_string()
    }
}

#[must_use]
pub fn get_error_code(status_code: StatusCode) -> Option<String> {
    if status_code == StatusCode::UNAUTHORIZED
        || status_code == StatusCode::FORBIDDEN
    {
        Some("invalid_api_key".to_string())
    } else if status_code == StatusCode::TOO_MANY_REQUESTS {
        Some("rate_limit_exceeded".to_string())
    } else {
        None
    }
}
