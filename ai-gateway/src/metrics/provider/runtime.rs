use std::sync::{Arc, Mutex};

use chrono::{DateTime, Utc};
use opentelemetry::metrics::{Counter, Histogram, Meter};

use super::{
    attempt::{CallOutcome, UsageSource},
    reservoir::LatencyReservoir,
    usage_json::GatewayProviderUsage,
};
use crate::metrics::llm::TokenUsage;

const RESERVOIR_CAP: usize = 1024;

#[derive(Debug, Clone)]
pub struct GatewayProviderMetrics {
    pub calls: Counter<u64>,
    pub responses_by_status: Counter<u64>,
    pub tokens: Counter<u64>,
    pub request_duration_ms: Histogram<f64>,
    pub tfft_ms: Histogram<f64>,
    pub generation_ms_per_output_token: Histogram<f64>,
    pub runtime: Arc<ProviderRuntimeRegistry>,
}

#[derive(Debug)]
pub struct ProviderRuntimeRegistry {
    started_at: DateTime<Utc>,
    routing: Mutex<RoutingTotals>,
    providers:
        Mutex<rustc_hash::FxHashMap<(String, String), ProviderRuntimeEntry>>,
}

#[derive(Debug, Default)]
struct RoutingTotals {
    client_requests: u64,
    requests_with_failover: u64,
}

#[derive(Debug)]
struct ProviderRuntimeEntry {
    calls: CallTotals,
    status_codes: rustc_hash::FxHashMap<u16, u64>,
    tokens: TokenTotals,
    duration_sum_ms: u64,
    duration_count: u64,
    tfft_sum_ms: u64,
    tfft_count: u64,
    generation_reservoir: LatencyReservoir,
    last_call_at: Option<DateTime<Utc>>,
    last_error_at: Option<DateTime<Utc>>,
    last_status_code: Option<u16>,
}

#[derive(Debug, Default)]
struct CallTotals {
    attempts: u64,
    success: u64,
    success_degraded: u64,
    client_error: u64,
    server_error: u64,
}

#[derive(Debug, Default, Clone)]
struct TokenTotals {
    input: u64,
    output: u64,
    cached: u64,
    reasoning: u64,
    estimated_input: u64,
    estimated_output: u64,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ProviderStatsSnapshot {
    pub started_at: DateTime<Utc>,
    pub uptime_seconds: u64,
    pub providers: Vec<ProviderStatsRow>,
    pub routing: RoutingSnapshot,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ProviderStatsRow {
    pub provider: String,
    pub credential: String,
    pub calls: ProviderCallSnapshot,
    pub status_codes: std::collections::BTreeMap<String, u64>,
    pub tokens: TokenSnapshot,
    pub latency: LatencySnapshot,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_call_at: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_error_at: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_status_code: Option<u16>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ProviderCallSnapshot {
    pub attempts: u64,
    pub success: u64,
    pub success_degraded: u64,
    pub client_error: u64,
    pub server_error: u64,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct TokenSnapshot {
    pub input: u64,
    pub output: u64,
    pub cached: u64,
    pub reasoning: u64,
    pub estimated_input: u64,
    pub estimated_output: u64,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct LatencySnapshot {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avg_duration_ms: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avg_tfft_ms: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avg_generation_ms_per_output_token: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub p50_generation_ms_per_output_token: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub p95_generation_ms_per_output_token: Option<f64>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct RoutingSnapshot {
    pub client_requests: u64,
    pub requests_with_failover: u64,
    pub failover_rate: f64,
}

pub struct AttemptRecord {
    pub provider: String,
    pub credential: String,
    pub model: String,
    pub router_id: String,
    pub attempt_index: u32,
    pub upstream_attempts: u32,
    pub status_code: u16,
    pub stream: bool,
    pub request_kind: &'static str,
    pub duration_ms: f64,
    pub tfft_ms: Option<f64>,
    pub usage: TokenUsage,
    pub usage_source: UsageSource,
    pub outcome: CallOutcome,
    pub overload: bool,
}

impl GatewayProviderMetrics {
    #[must_use]
    pub fn new(meter: &Meter) -> Self {
        Self {
            calls: meter
                .u64_counter("gateway_provider_calls_total")
                .with_description("Upstream provider attempts by outcome")
                .build(),
            responses_by_status: meter
                .u64_counter("gateway_provider_responses_by_status_total")
                .with_description("Upstream HTTP status codes per provider")
                .build(),
            tokens: meter
                .u64_counter("gateway_provider_tokens_total")
                .with_unit("{token}")
                .with_description("Tokens per upstream attempt")
                .build(),
            request_duration_ms: meter
                .f64_histogram("gateway_provider_request_duration_ms")
                .with_unit("ms")
                .with_description("Upstream attempt wall duration")
                .build(),
            tfft_ms: meter
                .f64_histogram("gateway_provider_tfft_ms")
                .with_unit("ms")
                .with_description("Upstream time to first token")
                .build(),
            generation_ms_per_output_token: meter
                .f64_histogram(
                    "gateway_provider_generation_ms_per_output_token",
                )
                .with_unit("ms")
                .with_description("Generation ms per output token")
                .build(),
            runtime: Arc::new(ProviderRuntimeRegistry::new()),
        }
    }

    pub fn record_attempt(&self, record: &AttemptRecord) {
        let attrs = attempt_attrs(record);
        self.calls.add(1, &attrs);
        if record.outcome == CallOutcome::RateLimited {
            let mut rate_attrs = attrs.clone();
            rate_attrs.push(opentelemetry::KeyValue::new(
                "outcome",
                CallOutcome::ClientError.as_str(),
            ));
            self.calls.add(1, &rate_attrs);
        }
        if record.outcome == CallOutcome::Overload {
            let mut server_attrs = attrs.clone();
            server_attrs.push(opentelemetry::KeyValue::new(
                "outcome",
                CallOutcome::ServerError.as_str(),
            ));
            self.calls.add(1, &server_attrs);
        }

        let mut status_attrs = base_attrs(record);
        status_attrs.push(opentelemetry::KeyValue::new(
            "status_code",
            i64::from(record.status_code),
        ));
        self.responses_by_status.add(1, &status_attrs);

        self.request_duration_ms
            .record(record.duration_ms, &base_attrs(record));
        if let Some(ttft) = record.tfft_ms {
            self.tfft_ms.record(ttft, &base_attrs(record));
        }
        if let Some(generation_ms) = generation_ms_per_output_token(record) {
            self.generation_ms_per_output_token
                .record(generation_ms, &base_attrs(record));
        }

        for (token_type, value) in record.usage.reported_values() {
            let mut token_attrs = base_attrs(record);
            token_attrs
                .push(opentelemetry::KeyValue::new("token_type", token_type));
            token_attrs.push(opentelemetry::KeyValue::new(
                "usage_source",
                usage_source_label(record.usage_source),
            ));
            self.tokens.add(value, &token_attrs);
        }

        self.runtime
            .record_attempt(record, generation_ms_per_output_token(record));
    }

    pub fn record_client_request(&self, had_failover: bool) {
        self.runtime.record_client_request(had_failover);
    }

    #[must_use]
    pub fn snapshot(
        &self,
        provider: Option<&str>,
        credential: Option<&str>,
    ) -> ProviderStatsSnapshot {
        self.runtime.snapshot(provider, credential)
    }
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss
)]
impl ProviderRuntimeRegistry {
    fn new() -> Self {
        Self {
            started_at: Utc::now(),
            routing: Mutex::new(RoutingTotals::default()),
            providers: Mutex::new(rustc_hash::FxHashMap::default()),
        }
    }

    fn record_client_request(&self, had_failover: bool) {
        let mut routing = self.routing.lock().expect("routing totals");
        routing.client_requests = routing.client_requests.saturating_add(1);
        if had_failover {
            routing.requests_with_failover =
                routing.requests_with_failover.saturating_add(1);
        }
    }

    fn record_attempt(
        &self,
        record: &AttemptRecord,
        generation_ms: Option<f64>,
    ) {
        let mut providers = self.providers.lock().expect("provider stats");
        let entry = providers
            .entry((record.provider.clone(), record.credential.clone()))
            .or_default();
        entry.calls.attempts = entry.calls.attempts.saturating_add(1);
        match record.outcome {
            CallOutcome::Success => {
                entry.calls.success = entry.calls.success.saturating_add(1);
            }
            CallOutcome::SuccessDegraded => {
                entry.calls.success_degraded =
                    entry.calls.success_degraded.saturating_add(1);
            }
            CallOutcome::ClientError | CallOutcome::RateLimited => {
                entry.calls.client_error =
                    entry.calls.client_error.saturating_add(1);
            }
            CallOutcome::ServerError | CallOutcome::Overload => {
                entry.calls.server_error =
                    entry.calls.server_error.saturating_add(1);
            }
        }
        *entry.status_codes.entry(record.status_code).or_insert(0) += 1;
        accumulate_tokens(
            &mut entry.tokens,
            &record.usage,
            record.usage_source,
        );
        entry.duration_sum_ms = entry
            .duration_sum_ms
            .saturating_add(record.duration_ms as u64);
        entry.duration_count = entry.duration_count.saturating_add(1);
        if let Some(ttft) = record.tfft_ms {
            entry.tfft_sum_ms = entry.tfft_sum_ms.saturating_add(ttft as u64);
            entry.tfft_count = entry.tfft_count.saturating_add(1);
        }
        if let Some(generation_ms) = generation_ms {
            entry.generation_reservoir.record(generation_ms);
        }
        let now = Utc::now();
        entry.last_call_at = Some(now);
        entry.last_status_code = Some(record.status_code);
        if record.status_code >= 400 {
            entry.last_error_at = Some(now);
        }
    }

    fn snapshot(
        &self,
        provider: Option<&str>,
        credential: Option<&str>,
    ) -> ProviderStatsSnapshot {
        let routing = self.routing.lock().expect("routing totals").clone();
        let providers = self.providers.lock().expect("provider stats");
        let started_at = self.started_at;
        let uptime_seconds = (Utc::now() - started_at)
            .num_seconds()
            .max(0)
            .cast_unsigned();
        let failover_rate = if routing.client_requests == 0 {
            0.0
        } else {
            routing.requests_with_failover as f64
                / routing.client_requests as f64
        };

        let mut rows = Vec::new();
        for ((prov, cred), entry) in providers.iter() {
            if provider.is_some_and(|p| p != prov) {
                continue;
            }
            if credential.is_some_and(|c| c != cred) {
                continue;
            }
            rows.push(entry.to_row(prov.clone(), cred.clone()));
        }
        rows.sort_by(|a, b| a.provider.cmp(&b.provider));

        ProviderStatsSnapshot {
            started_at,
            uptime_seconds,
            providers: rows,
            routing: RoutingSnapshot {
                client_requests: routing.client_requests,
                requests_with_failover: routing.requests_with_failover,
                failover_rate,
            },
        }
    }
}

#[allow(clippy::cast_precision_loss)]
impl ProviderRuntimeEntry {
    fn to_row(&self, provider: String, credential: String) -> ProviderStatsRow {
        let avg_duration_ms = (self.duration_count > 0).then_some(
            self.duration_sum_ms as f64 / self.duration_count as f64,
        );
        let avg_tfft_ms = (self.tfft_count > 0)
            .then_some(self.tfft_sum_ms as f64 / self.tfft_count as f64);
        let avg_generation = self.generation_reservoir.average();
        ProviderStatsRow {
            provider,
            credential,
            calls: ProviderCallSnapshot {
                attempts: self.calls.attempts,
                success: self.calls.success,
                success_degraded: self.calls.success_degraded,
                client_error: self.calls.client_error,
                server_error: self.calls.server_error,
            },
            status_codes: self
                .status_codes
                .iter()
                .map(|(code, count)| (code.to_string(), *count))
                .collect(),
            tokens: TokenSnapshot {
                input: self.tokens.input,
                output: self.tokens.output,
                cached: self.tokens.cached,
                reasoning: self.tokens.reasoning,
                estimated_input: self.tokens.estimated_input,
                estimated_output: self.tokens.estimated_output,
            },
            latency: LatencySnapshot {
                avg_duration_ms,
                avg_tfft_ms,
                avg_generation_ms_per_output_token: avg_generation,
                p50_generation_ms_per_output_token: self
                    .generation_reservoir
                    .percentile(50.0),
                p95_generation_ms_per_output_token: self
                    .generation_reservoir
                    .percentile(95.0),
            },
            last_call_at: self.last_call_at,
            last_error_at: self.last_error_at,
            last_status_code: self.last_status_code,
        }
    }
}

impl Default for ProviderRuntimeEntry {
    fn default() -> Self {
        Self {
            calls: CallTotals::default(),
            status_codes: rustc_hash::FxHashMap::default(),
            tokens: TokenTotals::default(),
            duration_sum_ms: 0,
            duration_count: 0,
            tfft_sum_ms: 0,
            tfft_count: 0,
            generation_reservoir: LatencyReservoir::new(RESERVOIR_CAP),
            last_call_at: None,
            last_error_at: None,
            last_status_code: None,
        }
    }
}

impl Clone for RoutingTotals {
    fn clone(&self) -> Self {
        Self {
            client_requests: self.client_requests,
            requests_with_failover: self.requests_with_failover,
        }
    }
}

fn accumulate_tokens(
    totals: &mut TokenTotals,
    usage: &TokenUsage,
    source: UsageSource,
) {
    let estimated = source == UsageSource::Estimated;
    if let Some(v) = usage.input {
        totals.input = totals.input.saturating_add(v);
        if estimated {
            totals.estimated_input = totals.estimated_input.saturating_add(v);
        }
    }
    if let Some(v) = usage.output {
        totals.output = totals.output.saturating_add(v);
        if estimated {
            totals.estimated_output = totals.estimated_output.saturating_add(v);
        }
    }
    if let Some(v) = usage.cached {
        totals.cached = totals.cached.saturating_add(v);
    }
    if let Some(v) = usage.reasoning {
        totals.reasoning = totals.reasoning.saturating_add(v);
    }
}

#[must_use]
#[allow(clippy::cast_precision_loss)]
pub fn generation_ms_per_output_token(record: &AttemptRecord) -> Option<f64> {
    let output = record.usage.output?;
    if output == 0 {
        return None;
    }
    let tfft = record.tfft_ms.unwrap_or(0.0);
    let gen_ms = record.duration_ms - tfft;
    if gen_ms <= 0.0 {
        return None;
    }
    Some(gen_ms / output as f64)
}

fn base_attrs(record: &AttemptRecord) -> Vec<opentelemetry::KeyValue> {
    vec![
        opentelemetry::KeyValue::new("provider", record.provider.clone()),
        opentelemetry::KeyValue::new("credential", record.credential.clone()),
        opentelemetry::KeyValue::new("model", record.model.clone()),
        opentelemetry::KeyValue::new("router_id", record.router_id.clone()),
        opentelemetry::KeyValue::new(
            "attempt_index",
            i64::from(record.attempt_index),
        ),
        opentelemetry::KeyValue::new("stream", record.stream),
        opentelemetry::KeyValue::new("request_kind", record.request_kind),
    ]
}

fn attempt_attrs(record: &AttemptRecord) -> Vec<opentelemetry::KeyValue> {
    let mut attrs = base_attrs(record);
    attrs.push(opentelemetry::KeyValue::new(
        "outcome",
        record.outcome.as_str(),
    ));
    attrs
}

fn usage_source_label(source: UsageSource) -> &'static str {
    match source {
        UsageSource::Reported => "reported",
        UsageSource::Estimated => "estimated",
        UsageSource::None => "none",
    }
}

#[must_use]
pub fn build_usage_header(
    record: &AttemptRecord,
    generation_ms: Option<f64>,
) -> GatewayProviderUsage {
    GatewayProviderUsage {
        provider: record.provider.clone(),
        credential: Some(record.credential.clone()).filter(|c| c != "default"),
        model: (record.model != "unknown").then_some(record.model.clone()),
        usage: UsageBlock {
            input: record.usage.input,
            output: record.usage.output,
            cached: record.usage.cached.filter(|v| *v > 0),
            reasoning: record.usage.reasoning.filter(|v| *v > 0),
            total: record.usage.total.or_else(|| {
                Some(
                    record
                        .usage
                        .input
                        .unwrap_or(0)
                        .saturating_add(record.usage.output.unwrap_or(0)),
                )
            }),
            source: usage_source_label(record.usage_source),
        },
        latency_ms: LatencyBlock {
            total: record.duration_ms,
            ttft: record.tfft_ms.filter(|_| record.stream),
            generation_per_output_token: generation_ms,
        },
        routing: RoutingBlock {
            attempts: record.upstream_attempts.max(1),
            failover: record.upstream_attempts > 1,
        },
    }
}

use super::usage_json::{LatencyBlock, RoutingBlock, UsageBlock};
