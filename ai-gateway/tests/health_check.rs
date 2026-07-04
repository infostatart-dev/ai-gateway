use std::{collections::HashMap, sync::Arc};

use ai_gateway::{
    config::{Config, helicone::HeliconeFeatures},
    tests::{TestDefault, harness::Harness, mock::MockArgs},
};
use http::{Method, Request, StatusCode, header::CONTENT_TYPE};
use http_body_util::BodyExt;
use opentelemetry::{global, metrics::MeterProvider};
use serde_json::Value;
use tower::Service;

#[derive(Clone)]
struct SharedMeterProvider(Arc<dyn MeterProvider + Send + Sync>);

impl MeterProvider for SharedMeterProvider {
    fn meter_with_scope(
        &self,
        scope: opentelemetry::InstrumentationScope,
    ) -> opentelemetry::metrics::Meter {
        self.0.meter_with_scope(scope)
    }
}

struct GlobalMeterRestore(Arc<dyn MeterProvider + Send + Sync>);

impl Drop for GlobalMeterRestore {
    fn drop(&mut self) {
        global::set_meter_provider(SharedMeterProvider(self.0.clone()));
    }
}

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
    assert!(
        health
            .get("started_at_server_timezone")
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

#[tokio::test]
#[serial_test::serial]
async fn metrics_endpoint_exports_prometheus_text() {
    let previous = global::meter_provider();
    let sdk = telemetry::init_prometheus_metrics_for_current_process(
        "ai-gateway-test",
    )
    .expect("initialize prometheus metrics");
    let _restore = GlobalMeterRestore(previous);

    let meter = global::meter("ai-gateway-test");
    let scrape_probe = meter.u64_counter("test_metrics_scrapes").build();
    scrape_probe.add(1, &[]);

    let mut config = Config::test_default();
    config.helicone.features = HeliconeFeatures::Auth;

    let mock_args = MockArgs::builder()
        .stubs(HashMap::from([
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
        .uri("http://router.helicone.com/metrics")
        .body(axum_core::body::Body::empty())
        .unwrap();

    let response = harness.call(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let content_type = response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_string();
    let body = response.into_body().collect().await.unwrap();
    let payload = String::from_utf8(body.to_bytes().to_vec()).unwrap();
    assert!(content_type.starts_with("text/plain; version=0.0.4"));
    assert!(payload.contains("test_metrics_scrapes_total"));
    assert!(payload.contains("target_info"));

    let _ = sdk.shutdown();
}

#[tokio::test]
#[serial_test::serial]
async fn models_endpoint_returns_declared_stable_and_unstable_models() {
    let mut config = Config::test_default();
    config.helicone.features = HeliconeFeatures::Auth;

    let mock_args = MockArgs::builder()
        .stubs(HashMap::from([
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
        .uri("http://router.helicone.com/models")
        .body(axum_core::body::Body::empty())
        .unwrap();

    let response = harness.call(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let content_type = response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_string();
    let body = response.into_body().collect().await.unwrap();
    let payload: Value = serde_json::from_slice(&body.to_bytes()).unwrap();

    assert_eq!(content_type, "application/json");
    assert!(payload["stable"].as_array().is_some_and(|models| {
        models.iter().any(|model| {
            model.get("id").and_then(Value::as_str)
                == Some("openai/gpt-5.5-nano")
        }) && models.iter().any(|model| {
            model.get("id").and_then(Value::as_str)
                == Some("openai/gpt-5.5-mini")
        })
    }));
    assert!(payload["unstable"].as_array().is_some_and(|models| {
        models.iter().any(|model| {
            model.get("id").and_then(Value::as_str) == Some("openai/gpt-5.5")
        })
    }));
}
