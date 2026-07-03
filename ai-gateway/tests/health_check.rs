use std::collections::HashMap;

use ai_gateway::{
    config::{Config, helicone::HeliconeFeatures},
    tests::{TestDefault, harness::Harness, mock::MockArgs},
};
use http::{Method, Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::Value;
use tower::Service;

#[tokio::test]
#[serial_test::serial]
async fn health_check() {
    let mut config = Config::test_default();
    config.helicone.features = HeliconeFeatures::Auth;

    let mock_args = MockArgs::builder()
        .stubs(HashMap::from([
            ("success:openai:chat_completion", 0.into()),
            ("success:anthropic:messages", 0.into()),
            ("success:minio:upload_request", 0.into()),
            ("success:jawn:log_request", 0.into()),
        ]))
        .build();
    let mut harness = Harness::builder()
        .with_config(config)
        .with_mock_args(mock_args)
        .build()
        .await;

    let request = Request::builder()
        .method(Method::GET)
        .uri("http://router.helicone.com/health")
        .body(axum_core::body::Body::empty())
        .unwrap();

    let response = harness.call(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap();
    let health: Value = serde_json::from_slice(&body.to_bytes()).unwrap();
    assert!(health.get("version").and_then(Value::as_str).is_some());
    assert!(
        health
            .get("started_at_utc")
            .and_then(Value::as_str)
            .is_some()
    );
    assert!(
        health
            .get("started_at_server_time")
            .and_then(Value::as_str)
            .is_some()
    );

    let request = Request::builder()
        .method(Method::GET)
        .uri("http://router.helicone.com/not-health-check")
        .body(axum_core::body::Body::empty())
        .unwrap();

    let response = harness.call(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}
