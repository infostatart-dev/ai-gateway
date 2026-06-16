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

/// Attributes for a single failover attempt between upstream candidates.
pub struct FailoverEvent<'a> {
    pub router_id: &'a RouterId,
    pub endpoint_type: &'a str,
    pub strategy: &'static str,
    pub from_provider: &'a InferenceProvider,
    pub to_provider: Option<&'a InferenceProvider>,
    pub reason: &'a str,
    pub credential: &'a str,
    pub quota_metric: &'a str,
}

/// Identity of a provider/credential cooldown transition.
pub struct CooldownEvent<'a> {
    pub router_id: &'a RouterId,
    pub endpoint_type: &'a str,
    pub strategy: &'static str,
    pub provider: &'a InferenceProvider,
    pub credential: &'a str,
}

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

    pub fn record_failover(&self, event: &FailoverEvent<'_>) {
        let attrs = [
            KeyValue::new("router_id", event.router_id.to_string()),
            KeyValue::new("endpoint_type", event.endpoint_type.to_string()),
            KeyValue::new("strategy", event.strategy),
            KeyValue::new("from_provider", event.from_provider.to_string()),
            KeyValue::new(
                "to_provider",
                event
                    .to_provider
                    .map_or_else(|| "unknown".to_string(), ToString::to_string),
            ),
            KeyValue::new("reason", event.reason.to_string()),
            KeyValue::new("credential", event.credential.to_string()),
            KeyValue::new("quota_metric", event.quota_metric.to_string()),
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
        event: &CooldownEvent<'_>,
        status_class: &'static str,
        quota_metric: &str,
    ) {
        let mut attrs = event.base_kv();
        attrs.push(KeyValue::new("status_class", status_class));
        attrs.push(KeyValue::new("quota_metric", quota_metric.to_string()));
        attrs.push(KeyValue::new("event", "enter"));
        self.router_provider_cooldown_events_total.add(1, &attrs);
        self.router_provider_in_cooldown
            .record(1, &event.gauge_kv());
    }

    pub fn record_cooldown_exit(&self, event: &CooldownEvent<'_>) {
        let mut attrs = event.base_kv();
        attrs.push(KeyValue::new("status_class", "recovered"));
        attrs.push(KeyValue::new("event", "exit"));
        self.router_provider_cooldown_events_total.add(1, &attrs);
        self.router_provider_in_cooldown
            .record(0, &event.gauge_kv());
    }
}

impl CooldownEvent<'_> {
    fn base_kv(&self) -> Vec<KeyValue> {
        let mut attrs = self.gauge_kv();
        attrs.push(KeyValue::new("credential", self.credential.to_string()));
        attrs
    }

    fn gauge_kv(&self) -> Vec<KeyValue> {
        vec![
            KeyValue::new("router_id", self.router_id.to_string()),
            KeyValue::new("endpoint_type", self.endpoint_type.to_string()),
            KeyValue::new("strategy", self.strategy),
            KeyValue::new("provider", self.provider.to_string()),
        ]
    }
}
