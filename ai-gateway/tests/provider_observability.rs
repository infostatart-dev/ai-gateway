//! Provider observability: REST snapshot and `X-Gateway-Provider-Usage` header.

use std::{collections::HashMap, time::Duration};

use ai_gateway::{
    app::AppResponse,
    config::{
        Config,
        helicone::HeliconeFeatures,
        observability::{
            ObservabilityConfig, ObservabilityResponseHeadersConfig,
        },
    },
    tests::{TestDefault, harness::Harness, mock::MockArgs},
};
use http::{Method, Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::Service;

async fn drain(response: AppResponse) {
    let _ = response.into_body().collect().await.unwrap();
}

async fn post_json(
    harness: &mut Harness,
    uri: &str,
    body: Value,
) -> AppResponse {
    let request = Request::builder()
        .method(Method::POST)
        .uri(uri)
        .header("content-type", "application/json")
        .body(axum_core::body::Body::from(
            serde_json::to_vec(&body).unwrap(),
        ))
        .unwrap();
    harness.call(request).await.unwrap()
}

fn provider_row<'a>(snapshot: &'a Value, provider: &str) -> Option<&'a Value> {
    snapshot.get("providers")?.as_array()?.iter().find(|row| {
        row.get("provider").and_then(Value::as_str) == Some(provider)
    })
}

fn attempts(snapshot: &Value, provider: &str) -> u64 {
    provider_row(snapshot, provider)
        .and_then(|row| row.get("calls"))
        .and_then(|calls| calls.get("attempts"))
        .and_then(Value::as_u64)
        .unwrap_or(0)
}

fn credential_row<'a>(
    snapshot: &'a Value,
    provider: &str,
    credential: &str,
) -> Option<&'a Value> {
    snapshot.get("providers")?.as_array()?.iter().find(|row| {
        row.get("provider").and_then(Value::as_str) == Some(provider)
            && row.get("credential").and_then(Value::as_str) == Some(credential)
    })
}

async fn fetch_stats(harness: &mut Harness, path: &str) -> Value {
    let request = Request::builder()
        .method(Method::GET)
        .uri(format!("http://router.helicone.com{path}"))
        .body(axum_core::body::Body::empty())
        .unwrap();
    let response = harness.call(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap();
    serde_json::from_slice(&body.to_bytes()).unwrap()
}

#[tokio::test]
#[serial_test::serial(default_mock)]
async fn provider_stats_public_and_aggregates_two_providers() {
    let mut config = Config::test_default();
    config.helicone.features = HeliconeFeatures::None;

    let mock_args = MockArgs::builder()
        .stubs(HashMap::from([
            ("success:openai:chat_completion", 1.into()),
            ("success:anthropic:fake_endpoint", 1.into()),
            ("success:minio:upload_request", 0.into()),
            ("success:jawn:log_request", 0.into()),
        ]))
        .build();
    let mut harness = Harness::builder()
        .with_config(config)
        .with_mock_args(mock_args)
        .build()
        .await;

    let openai = post_json(
        &mut harness,
        "http://router.helicone.com/router/my-router/chat/completions",
        json!({
            "model": "openai/gpt-4o-mini",
            "messages": [{"role": "user", "content": "Hello"}]
        }),
    )
    .await;
    assert_eq!(openai.status(), StatusCode::OK);
    drain(openai).await;

    let anthropic = post_json(
        &mut harness,
        "http://router.helicone.com/anthropic/v1/fake_endpoint",
        json!({"probe": true}),
    )
    .await;
    assert_eq!(anthropic.status(), StatusCode::OK);
    drain(anthropic).await;

    tokio::time::sleep(Duration::from_millis(300)).await;

    let snapshot =
        fetch_stats(&mut harness, "/v1/observability/provider-stats").await;
    assert!(snapshot.get("version").and_then(Value::as_str).is_some());
    assert!(snapshot.get("started_at").is_some());
    assert!(
        snapshot
            .get("started_at_utc")
            .and_then(Value::as_str)
            .is_some()
    );
    assert!(
        snapshot
            .get("started_at_server_time")
            .and_then(Value::as_str)
            .is_some()
    );
    assert!(snapshot.get("uptime_seconds").is_some());
    assert!(
        attempts(&snapshot, "openai") >= 1,
        "expected openai attempts"
    );
    assert!(
        attempts(&snapshot, "anthropic") >= 1,
        "expected anthropic attempts"
    );

    let filtered =
        fetch_stats(&mut harness, "/v1/observability/provider-stats/openai")
            .await;
    let providers = filtered
        .get("providers")
        .and_then(Value::as_array)
        .expect("providers array");
    assert!(
        providers
            .iter()
            .all(|row| row.get("provider").and_then(Value::as_str)
                == Some("openai")),
        "provider filter should restrict rows"
    );

    let authed_probe = Request::builder()
        .method(Method::GET)
        .uri("http://router.helicone.com/v1/observability/provider-stats")
        .body(axum_core::body::Body::empty())
        .unwrap();
    let response = harness.call(authed_probe).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
#[serial_test::serial(default_mock)]
async fn provider_stats_includes_idle_configured_credentials() {
    let mut config = Config::test_default();
    config.helicone.features = HeliconeFeatures::None;

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

    let snapshot =
        fetch_stats(&mut harness, "/v1/observability/provider-stats").await;
    let providers = snapshot
        .get("providers")
        .and_then(Value::as_array)
        .expect("providers array");
    assert!(
        !providers.is_empty(),
        "configured credentials should appear as idle rows"
    );
    let idle = providers
        .iter()
        .filter(|row| row.get("status").and_then(Value::as_str) == Some("idle"))
        .count();
    assert!(
        idle > 0,
        "at least one credential should be idle before any traffic"
    );
    let row = credential_row(&snapshot, "openai", "default")
        .or_else(|| credential_row(&snapshot, "openai", "openai-default"));
    if let Some(row) = row {
        assert_eq!(row.get("status").and_then(Value::as_str), Some("idle"));
        assert_eq!(
            row.get("calls")
                .and_then(|c| c.get("attempts"))
                .and_then(Value::as_u64),
            Some(0)
        );
        let health = row.get("routing_health").expect("routing_health object");
        assert_eq!(health.get("circuit_open"), Some(&json!(false)));
        assert_eq!(health.get("planner_excluded"), Some(&json!(false)));
        assert!(health.get("success_rate").is_some());
    }
}

#[tokio::test]
#[serial_test::serial(default_mock)]
async fn provider_stats_includes_quota_tree() {
    let mut config = Config::test_default();
    config.helicone.features = HeliconeFeatures::None;

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

    let snapshot =
        fetch_stats(&mut harness, "/v1/observability/provider-stats").await;
    assert_eq!(
        snapshot
            .get("routing")
            .and_then(|r| r.get("repeat_429_violations"))
            .and_then(Value::as_u64),
        Some(0)
    );
    let providers = snapshot
        .get("providers")
        .and_then(Value::as_array)
        .expect("providers array");
    let enriched = providers
        .iter()
        .find(|row| row.get("quota_profile").is_some())
        .expect("at least one row should include quota_profile after enrich");
    assert!(
        enriched
            .get("quota_profile")
            .and_then(Value::as_str)
            .is_some()
    );
    let quota = snapshot
        .get("quota")
        .and_then(Value::as_array)
        .expect("quota tree");
    assert!(!quota.is_empty(), "quota tree should list providers");
    let accounts = quota
        .iter()
        .find_map(|node| node.get("accounts").and_then(Value::as_array))
        .expect("quota node with accounts");
    assert!(!accounts.is_empty());
    assert!(
        accounts[0]
            .get("credential_id")
            .and_then(Value::as_str)
            .is_some()
    );
}

#[tokio::test]
#[serial_test::serial(default_mock)]
async fn usage_header_reported_on_router_completion() {
    let mut config = Config::test_default();
    config.helicone.features = HeliconeFeatures::None;

    let mock_args = MockArgs::builder()
        .stubs(HashMap::from([
            ("success:openai:chat_completion", 1.into()),
            ("success:minio:upload_request", 0.into()),
            ("success:jawn:log_request", 0.into()),
        ]))
        .build();
    let mut harness = Harness::builder()
        .with_config(config)
        .with_mock_args(mock_args)
        .build()
        .await;

    let response = post_json(
        &mut harness,
        "http://router.helicone.com/router/my-router/chat/completions",
        json!({
            "model": "openai/gpt-4o-mini",
            "messages": [{"role": "user", "content": "Hello"}]
        }),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);

    let header = response
        .headers()
        .get("x-gateway-provider-usage")
        .expect("usage header");
    let usage: Value = serde_json::from_str(header.to_str().unwrap()).unwrap();
    assert_eq!(
        usage.get("provider").and_then(Value::as_str),
        Some("openai")
    );
    assert_eq!(
        usage
            .get("usage")
            .and_then(|u| u.get("source"))
            .and_then(Value::as_str),
        Some("reported")
    );
    drain(response).await;
}

#[tokio::test]
#[serial_test::serial(default_mock)]
async fn usage_header_estimated_when_upstream_omits_usage() {
    let mut config = Config::test_default();
    config.helicone.features = HeliconeFeatures::None;

    let mock_args = MockArgs::builder()
        .stubs(HashMap::from([
            ("success:openai:fake_endpoint", 1.into()),
            ("success:minio:upload_request", 0.into()),
            ("success:jawn:log_request", 0.into()),
        ]))
        .build();
    let mut harness = Harness::builder()
        .with_config(config)
        .with_mock_args(mock_args)
        .build()
        .await;

    let response = post_json(
        &mut harness,
        "http://router.helicone.com/openai/v1/fake_endpoint",
        json!({
            "messages": [{"role": "user", "content": "estimate me"}]
        }),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);

    let header = response
        .headers()
        .get("x-gateway-provider-usage")
        .expect("usage header");
    let usage: Value = serde_json::from_str(header.to_str().unwrap()).unwrap();
    assert_eq!(
        usage
            .get("usage")
            .and_then(|u| u.get("source"))
            .and_then(Value::as_str),
        Some("estimated")
    );
    drain(response).await;
}

#[tokio::test]
#[serial_test::serial(default_mock)]
async fn usage_header_absent_when_disabled() {
    let mut config = Config::test_default();
    config.helicone.features = HeliconeFeatures::None;
    config.observability = ObservabilityConfig {
        estimate_tokens: true,
        response_headers: ObservabilityResponseHeadersConfig {
            enabled: false,
            echo_work_unit_id: true,
        },
    };

    let mock_args = MockArgs::builder()
        .stubs(HashMap::from([
            ("success:openai:chat_completion", 1.into()),
            ("success:minio:upload_request", 0.into()),
            ("success:jawn:log_request", 0.into()),
        ]))
        .build();
    let mut harness = Harness::builder()
        .with_config(config)
        .with_mock_args(mock_args)
        .build()
        .await;

    let response = post_json(
        &mut harness,
        "http://router.helicone.com/router/my-router/chat/completions",
        json!({
            "model": "openai/gpt-4o-mini",
            "messages": [{"role": "user", "content": "Hello"}]
        }),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert!(
        !response.headers().contains_key("x-gateway-provider-usage"),
        "header must be omitted when disabled"
    );
    drain(response).await;
}

#[tokio::test]
#[serial_test::serial(default_mock)]
async fn attempt_record_includes_agent_name_attribute() {
    use ai_gateway::{
        metrics::provider::{RecordAttemptInput, build_attempt_record},
        types::extensions::RequestKind,
    };

    let record = build_attempt_record(&RecordAttemptInput {
        provider: &ai_gateway::types::provider::InferenceProvider::OpenAI,
        credential: "default",
        model: None,
        router_id: None,
        attempt: None,
        status: http::StatusCode::OK,
        stream: false,
        request_kind: RequestKind::Router,
        duration_ms: 12.0,
        tfft_ms: None,
        reported_usage: ai_gateway::metrics::llm::TokenUsage::default(),
        request_body: None,
        estimate_tokens: false,
        failover_class: None,
        agent_name: Some("invoker-alpha"),
    });
    assert_eq!(record.agent_name.as_deref(), Some("invoker-alpha"));
}
