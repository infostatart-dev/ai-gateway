//! Integration: router_* OTEL instruments flush into an in-memory exporter.

use std::{collections::HashMap, sync::Arc, time::Duration};

use ai_gateway::{
    config::{Config, helicone::HeliconeFeatures},
    tests::{TestDefault, harness::Harness, mock::MockArgs},
};
use http::{Method, Request, StatusCode};
use opentelemetry::{KeyValue, global, metrics::MeterProvider};
use opentelemetry_sdk::metrics::{
    InMemoryMetricExporter, PeriodicReader, SdkMeterProvider,
    data::{AggregatedMetrics, MetricData, ResourceMetrics},
};
use serde_json::json;
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

fn sum_u64_metric(resources: &[ResourceMetrics], want_name: &str) -> u64 {
    let mut total = 0u64;
    for rm in resources {
        for sm in rm.scope_metrics() {
            for m in sm.metrics() {
                if m.name() != want_name {
                    continue;
                }
                let AggregatedMetrics::U64(data) = m.data() else {
                    continue;
                };
                let MetricData::Sum(sum) = data else {
                    continue;
                };
                for dp in sum.data_points() {
                    total += dp.value();
                }
            }
        }
    }
    total
}

fn has_kv(
    mut attrs: impl Iterator<Item = KeyValue>,
    key: &str,
    value_substr: &str,
) -> bool {
    attrs.any(|kv| {
        kv.key.as_str() == key
            && matches!(&kv.value, opentelemetry::Value::String(s) if s.as_str().contains(value_substr))
    })
}

fn tokens_datapoint_with_input(resources: &[ResourceMetrics]) -> bool {
    for rm in resources {
        for sm in rm.scope_metrics() {
            for m in sm.metrics() {
                if m.name() != "router_tokens_total" {
                    continue;
                }
                let AggregatedMetrics::U64(data) = m.data() else {
                    continue;
                };
                let MetricData::Sum(sum) = data else {
                    continue;
                };
                for dp in sum.data_points() {
                    if has_kv(dp.attributes().cloned(), "token_type", "input") {
                        return true;
                    }
                }
            }
        }
    }
    false
}

#[tokio::test]
#[serial_test::serial(default_mock)]
async fn router_counters_recorded_after_chat_completion() {
    let previous = global::meter_provider();
    let exporter = InMemoryMetricExporter::default();
    let reader = PeriodicReader::builder(exporter.clone()).build();
    let sdk = SdkMeterProvider::builder().with_reader(reader).build();
    global::set_meter_provider(sdk.clone());
    let _restore = GlobalMeterRestore(previous);

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

    let request_body = axum_core::body::Body::from(
        serde_json::to_vec(&json!({
            "model": "openai/gpt-4o-mini",
            "messages": [{"role": "user", "content": "Hello"}]
        }))
        .unwrap(),
    );
    let request = Request::builder()
        .method(Method::POST)
        .uri("http://router.helicone.com/router/my-router/chat/completions")
        .body(request_body)
        .unwrap();
    let response = harness.call(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // Dispatcher records router completion (incl. token usage) in a spawned
    // task after the response body is collected — wait before flush.
    tokio::time::sleep(Duration::from_millis(300)).await;

    sdk.force_flush().expect("flush OTEL metrics");

    let finished = exporter.get_finished_metrics().expect("read metrics");
    assert!(
        sum_u64_metric(&finished, "router_requests_total") >= 1,
        "expected router_requests_total"
    );
    assert!(
        tokens_datapoint_with_input(&finished),
        "expected router_tokens_total with token_type containing input"
    );

    let _ = sdk.shutdown();
}
