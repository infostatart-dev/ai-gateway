//! Router-level runtime OTEL metrics (`router_*`).

mod attrs;
pub(crate) mod events;
pub mod layer;
pub(crate) mod strategy_name;

#[cfg(test)]
mod tests;

pub use attrs::{base_router_kv, status_class};
pub use layer::RouterMetricsLayer;
use opentelemetry::metrics::{Counter, Gauge, Histogram, Meter};
pub use strategy_name::strategy_label;

use crate::metrics::llm::TokenUsage;

#[derive(Debug, Clone)]
pub struct RouterRuntimeMetrics {
    pub router_requests_total: Counter<u64>,
    pub router_responses_total: Counter<u64>,
    pub router_request_duration_ms: Histogram<f64>,
    pub router_tfft_duration_ms: Histogram<f64>,
    pub router_request_body_bytes_total: Counter<u64>,
    pub router_response_body_bytes_total: Counter<u64>,
    pub router_tokens_total: Counter<u64>,
    pub router_failover_attempts_total: Counter<u64>,
    pub router_retry_attempts_total: Counter<u64>,
    pub router_hedging_triggers_total: Counter<u64>,
    pub router_provider_cooldown_events_total: Counter<u64>,
    pub router_provider_in_cooldown: Gauge<u64>,
}

impl RouterRuntimeMetrics {
    #[must_use]
    pub fn new(meter: &Meter) -> Self {
        Self {
            router_requests_total: meter
                .u64_counter("router_requests_total")
                .with_description(
                    "Requests entering a logical router (per router_id)",
                )
                .build(),
            router_responses_total: meter
                .u64_counter("router_responses_total")
                .with_description("Responses returned from a logical router")
                .build(),
            router_request_duration_ms: meter
                .f64_histogram("router_request_duration_ms")
                .with_unit("ms")
                .with_description(
                    "End-to-end router latency until response body completes",
                )
                .build(),
            router_tfft_duration_ms: meter
                .f64_histogram("router_tfft_duration_ms")
                .with_unit("ms")
                .with_description("Time to first token for router requests")
                .build(),
            router_request_body_bytes_total: meter
                .u64_counter("router_request_body_bytes_total")
                .with_description("Bytes received in router requests")
                .build(),
            router_response_body_bytes_total: meter
                .u64_counter("router_response_body_bytes_total")
                .with_description(
                    "Bytes returned to clients from router responses",
                )
                .build(),
            router_tokens_total: meter
                .u64_counter("router_tokens_total")
                .with_description(
                    "Tokens attributed to a router and upstream provider",
                )
                .build(),
            router_failover_attempts_total: meter
                .u64_counter("router_failover_attempts_total")
                .with_description(
                    "Fail-over attempts between upstream candidates",
                )
                .build(),
            router_retry_attempts_total: meter
                .u64_counter("router_retry_attempts_total")
                .with_description(
                    "Inline dispatcher retries before returning/failover",
                )
                .build(),
            router_hedging_triggers_total: meter
                .u64_counter("router_hedging_triggers_total")
                .with_description("Hedged secondary requests triggered")
                .build(),
            router_provider_cooldown_events_total: meter
                .u64_counter("router_provider_cooldown_events_total")
                .with_description(
                    "Cooldown entries and exits per upstream provider",
                )
                .build(),
            router_provider_in_cooldown: meter
                .u64_gauge("router_provider_in_cooldown")
                .with_description(
                    "Whether a provider is currently in cooldown (1/0)",
                )
                .build(),
        }
    }

    /// Records usage rows for [`TokenUsage::reported_values`].
    pub fn record_router_tokens(
        &self,
        base: &[opentelemetry::KeyValue],
        provider: &crate::types::provider::InferenceProvider,
        model: Option<&crate::types::model_id::ModelId>,
        usage: TokenUsage,
    ) {
        for (token_type, value) in usage.reported_values() {
            let mut attrs = base.to_vec();
            attrs.push(opentelemetry::KeyValue::new(
                "provider",
                provider.to_string(),
            ));
            attrs.push(opentelemetry::KeyValue::new(
                "model",
                model
                    .map_or_else(|| "unknown".to_string(), ToString::to_string),
            ));
            attrs.push(opentelemetry::KeyValue::new("token_type", token_type));
            self.router_tokens_total.add(value, &attrs);
        }
    }
}
