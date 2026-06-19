use axum_core::body::Body;
use http::{Response, StatusCode};

pub type ResponseFactory = fn() -> Response<Body>;

#[must_use]
pub fn ok_chat_completion() -> Response<Body> {
    Response::builder()
        .status(StatusCode::OK)
        .header(http::header::CONTENT_TYPE, "application/json")
        .body(Body::from(r#"{"choices":[{"message":{"content":"ok"}}]}"#))
        .unwrap()
}

#[must_use]
pub fn ok_nano_json_schema_completion() -> Response<Body> {
    Response::builder()
        .status(StatusCode::OK)
        .header(http::header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            r#"{"choices":[{"message":{"content":"{\"ok\":true}"}}]}"#,
        ))
        .unwrap()
}

#[must_use]
pub fn ok_fat_json_schema_completion() -> Response<Body> {
    Response::builder()
        .status(StatusCode::OK)
        .header(http::header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            r#"{"choices":[{"message":{"content":"{\"value\":\"ok\",\"details\":\"routing load\"}"}}]}"#,
        ))
        .unwrap()
}

#[must_use]
pub fn rate_limited_rpm() -> Response<Body> {
    Response::builder()
        .status(StatusCode::TOO_MANY_REQUESTS)
        .body(Body::from(r#"{"error":"rate limit"}"#))
        .unwrap()
}

#[must_use]
pub fn project_billing_exhausted() -> Response<Body> {
    Response::builder()
        .status(StatusCode::TOO_MANY_REQUESTS)
        .body(Body::from(
            r#"{"error":{"message":"Set up billing to continue using this project."}}"#,
        ))
        .unwrap()
}

#[must_use]
pub fn daily_quota_exhausted() -> Response<Body> {
    Response::builder()
        .status(StatusCode::TOO_MANY_REQUESTS)
        .body(Body::from(
            r#"{"error":{"message":"You exceeded your daily limit."}}"#,
        ))
        .unwrap()
}

#[must_use]
pub fn overload_503() -> Response<Body> {
    Response::builder()
        .status(StatusCode::SERVICE_UNAVAILABLE)
        .body(Body::from("model is overloaded"))
        .unwrap()
}

#[must_use]
pub fn high_demand_503() -> Response<Body> {
    Response::builder()
        .status(StatusCode::SERVICE_UNAVAILABLE)
        .body(Body::from(
            "This model is currently experiencing high demand. Please try \
             again later.",
        ))
        .unwrap()
}

#[must_use]
pub fn not_found_404() -> Response<Body> {
    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .body(Body::from(
            r#"{"error":{"message":"models/gemini-3.5-flash-preview is not found"}}"#,
        ))
        .unwrap()
}

#[must_use]
pub fn openrouter_free_models_per_day_429() -> Response<Body> {
    Response::builder()
        .status(StatusCode::TOO_MANY_REQUESTS)
        .header("X-RateLimit-Reset", "999999999999")
        .body(Body::from(
            r#"{"error":{"message":"Rate limit exceeded: free-models-per-day"}}"#,
        ))
        .unwrap()
}

#[must_use]
pub fn openrouter_never_purchased_402() -> Response<Body> {
    Response::builder()
        .status(StatusCode::PAYMENT_REQUIRED)
        .body(Body::from(
            r#"{"error":{"message":"You have never purchased credits. Only free models are available."}}"#,
        ))
        .unwrap()
}

#[must_use]
pub fn credential_restricted(restricted_until: Option<&str>) -> Response<Body> {
    let until = restricted_until.unwrap_or("2026-06-19T09:34:11Z");
    Response::builder()
        .status(StatusCode::FORBIDDEN)
        .header(http::header::CONTENT_TYPE, "application/json")
        .body(Body::from(format!(
            r#"{{"error":{{"message":"user is muted","code":"credential_restricted","restricted_until":"{until}"}}}}"#
        )))
        .unwrap()
}

#[must_use]
pub fn credential_restricted_default() -> Response<Body> {
    credential_restricted(None)
}
