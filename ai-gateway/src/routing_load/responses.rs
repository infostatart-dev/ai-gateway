use axum_core::body::Body;
use http::StatusCode;

pub fn ok_chat_completion() -> crate::types::response::Response {
    http::Response::builder()
        .status(StatusCode::OK)
        .header(http::header::CONTENT_TYPE, "application/json")
        .body(Body::from(r#"{"choices":[{"message":{"content":"ok"}}]}"#))
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
