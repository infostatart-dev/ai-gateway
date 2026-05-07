use http::StatusCode;
use opentelemetry::KeyValue;

use super::{RouterRuntimeMetrics, attrs};
use crate::{
    metrics::llm::TokenUsage,
    types::{
        extensions::RouterRuntimeLabels, provider::InferenceProvider,
        router::RouterId,
    },
};

impl RouterRuntimeMetrics {
    #[allow(clippy::too_many_arguments)]
    pub fn record_router_complete(
        &self,
        rtl: &RouterRuntimeLabels,
        provider: &InferenceProvider,
        model: Option<&crate::types::model_id::ModelId>,
        status: StatusCode,
        req_body_len: usize,
        resp_body_len: usize,
        duration_wall_ms: f64,
        tfft_ms: Option<f64>,
        usage: TokenUsage,
        skip_upstream_body_metrics: bool,
    ) {
        let mut base = attrs::base_router_kv(rtl);
        base.push(KeyValue::new("status_code", i64::from(status.as_u16())));
        base.push(KeyValue::new("status_class", attrs::status_class(status)));

        self.router_responses_total.add(1, &base);
        self.router_request_duration_ms
            .record(duration_wall_ms, &base);
        self.router_request_body_bytes_total
            .add(u64::try_from(req_body_len).unwrap_or(u64::MAX), &base);

        if let Some(ms) = tfft_ms {
            self.router_tfft_duration_ms.record(ms, &base);
        }

        if !skip_upstream_body_metrics {
            self.router_response_body_bytes_total
                .add(u64::try_from(resp_body_len).unwrap_or(u64::MAX), &base);
            if !usage.is_empty() {
                let token_base = attrs::base_router_kv(rtl);
                self.record_router_tokens(&token_base, provider, model, usage);
            }
        }
    }

    pub fn record_failover(
        &self,
        router_id: &RouterId,
        endpoint_type: &str,
        strategy: &'static str,
        from_provider: &InferenceProvider,
        to_provider: Option<&InferenceProvider>,
        reason: &str,
    ) {
        let attrs = [
            KeyValue::new("router_id", router_id.to_string()),
            KeyValue::new("endpoint_type", endpoint_type.to_string()),
            KeyValue::new("strategy", strategy),
            KeyValue::new("from_provider", from_provider.to_string()),
            KeyValue::new(
                "to_provider",
                to_provider
                    .map_or_else(|| "unknown".to_string(), ToString::to_string),
            ),
            KeyValue::new("reason", reason.to_string()),
        ];
        self.router_failover_attempts_total.add(1, &attrs);
    }

    pub fn record_retry_attempt(
        &self,
        rtl: &RouterRuntimeLabels,
        provider: &InferenceProvider,
        reason: &'static str,
    ) {
        let mut attrs = attrs::base_router_kv(rtl);
        attrs.push(KeyValue::new("provider", provider.to_string()));
        attrs.push(KeyValue::new("reason", reason));
        self.router_retry_attempts_total.add(1, &attrs);
    }

    pub fn record_hedging_trigger(&self, rtl: &RouterRuntimeLabels) {
        let attrs = attrs::base_router_kv(rtl);
        self.router_hedging_triggers_total.add(1, &attrs);
    }

    pub fn record_cooldown_enter(
        &self,
        router_id: &RouterId,
        endpoint_type: &str,
        strategy: &'static str,
        provider: &InferenceProvider,
        status_class: &'static str,
    ) {
        let attrs = [
            KeyValue::new("router_id", router_id.to_string()),
            KeyValue::new("endpoint_type", endpoint_type.to_string()),
            KeyValue::new("strategy", strategy),
            KeyValue::new("provider", provider.to_string()),
            KeyValue::new("status_class", status_class),
            KeyValue::new("event", "enter"),
        ];
        self.router_provider_cooldown_events_total.add(1, &attrs);
        let gauge_attrs =
            cooldown_gauge_kv(router_id, endpoint_type, strategy, provider);
        self.router_provider_in_cooldown.record(1, &gauge_attrs);
    }

    pub fn record_cooldown_exit(
        &self,
        router_id: &RouterId,
        endpoint_type: &str,
        strategy: &'static str,
        provider: &InferenceProvider,
    ) {
        let attrs = [
            KeyValue::new("router_id", router_id.to_string()),
            KeyValue::new("endpoint_type", endpoint_type.to_string()),
            KeyValue::new("strategy", strategy),
            KeyValue::new("provider", provider.to_string()),
            KeyValue::new("status_class", "recovered"),
            KeyValue::new("event", "exit"),
        ];
        self.router_provider_cooldown_events_total.add(1, &attrs);
        let gauge_attrs =
            cooldown_gauge_kv(router_id, endpoint_type, strategy, provider);
        self.router_provider_in_cooldown.record(0, &gauge_attrs);
    }
}

fn cooldown_gauge_kv(
    router_id: &RouterId,
    endpoint_type: &str,
    strategy: &'static str,
    provider: &InferenceProvider,
) -> Vec<KeyValue> {
    vec![
        KeyValue::new("router_id", router_id.to_string()),
        KeyValue::new("endpoint_type", endpoint_type.to_string()),
        KeyValue::new("strategy", strategy),
        KeyValue::new("provider", provider.to_string()),
    ]
}
