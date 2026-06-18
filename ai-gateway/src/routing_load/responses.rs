use axum_core::body::Body;
use http::StatusCode;

pub fn ok_chat_completion() -> crate::types::response::Response {
    http::Response::builder()
        .status(StatusCode::OK)
        .header(http::header::CONTENT_TYPE, "application/json")
        .body(Body::from(r#"{"choices":[{"message":{"content":"ok"}}]}"#))
        .unwrap()
}

/// Valid assistant JSON for [`super::payload::nano_json_strict_body`].
pub fn ok_nano_json_schema_completion() -> crate::types::response::Response {
    http::Response::builder()
        .status(StatusCode::OK)
        .header(http::header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            r#"{"choices":[{"message":{"content":"{\"ok\":true}"}}]}"#,
        ))
        .unwrap()
}

/// Valid assistant JSON for [`super::payload::fat_json_schema_body`].
pub fn ok_fat_json_schema_completion() -> crate::types::response::Response {
    http::Response::builder()
        .status(StatusCode::OK)
        .header(http::header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            r#"{"choices":[{"message":{"content":"{\"value\":\"ok\",\"details\":\"routing load\"}"}}]}"#,
        ))
        .unwrap()
}

pub fn rate_limited_rpm() -> crate::types::response::Response {
    http::Response::builder()
        .status(StatusCode::TOO_MANY_REQUESTS)
        .body(Body::from(r#"{"error":"rate limit"}"#))
        .unwrap()
}

pub fn project_billing_exhausted() -> crate::types::response::Response {
    http::Response::builder()
        .status(StatusCode::TOO_MANY_REQUESTS)
        .body(Body::from(
            r#"{"error":{"message":"Set up billing to continue using this project."}}"#,
        ))
        .unwrap()
}

pub fn daily_quota_exhausted() -> crate::types::response::Response {
    http::Response::builder()
        .status(StatusCode::TOO_MANY_REQUESTS)
        .body(Body::from(
            r#"{"error":{"message":"You exceeded your daily limit."}}"#,
        ))
        .unwrap()
}

pub fn overload_503() -> crate::types::response::Response {
    http::Response::builder()
        .status(StatusCode::SERVICE_UNAVAILABLE)
        .body(Body::from("model is overloaded"))
        .unwrap()
}

pub fn high_demand_503() -> crate::types::response::Response {
    http::Response::builder()
        .status(StatusCode::SERVICE_UNAVAILABLE)
        .body(Body::from(
            "This model is currently experiencing high demand. Please try \
             again later.",
        ))
        .unwrap()
}

pub fn not_found_404() -> crate::types::response::Response {
    http::Response::builder()
        .status(StatusCode::NOT_FOUND)
        .body(Body::from(
            r#"{"error":{"message":"models/gemini-3.5-flash-preview is not found"}}"#,
        ))
        .unwrap()
}
