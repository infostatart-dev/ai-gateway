use std::{
    str::FromStr,
    sync::{Arc, Mutex},
};

use chrono::{DateTime, Local, Utc};
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
    pub repeat_429_violations: Counter<u64>,
    pub request_duration_ms: Histogram<f64>,
    pub tfft_ms: Histogram<f64>,
    pub generation_ms_per_output_token: Histogram<f64>,
    pub runtime: Arc<ProviderRuntimeRegistry>,
    pub health: Arc<crate::router::budget_aware::CredentialHealthRegistry>,
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
    repeat_429_violations: u64,
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
    semantic_error: u64,
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
    pub version: &'static str,
    pub started_at: DateTime<Utc>,
    pub started_at_utc: DateTime<Utc>,
    pub started_at_server_time: String,
    pub started_at_server_timezone: String,
    pub uptime_seconds: u64,
    pub providers: Vec<ProviderStatsRow>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub quota: Vec<super::quota_observability::QuotaProviderRow>,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub routing_health:
        Option<crate::router::budget_aware::RoutingHealthSnapshot>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quota_profile: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_available_at: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blocked_reason: Option<crate::router::quota_admission::BlockedReason>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub models: Option<Vec<super::quota_observability::QuotaModelRow>>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ProviderCallSnapshot {
    pub attempts: u64,
    pub success: u64,
    pub success_degraded: u64,
    pub semantic_error: u64,
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
    pub repeat_429_violations: u64,
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
    pub agent_name: Option<String>,
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
            repeat_429_violations: meter
                .u64_counter("gateway_repeat_429_violations_total")
                .with_description(
                    "Upstream 429 on scopes that were infeasible at hop admit \
                     time",
                )
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
            health: Arc::new(
                crate::router::budget_aware::CredentialHealthRegistry::new(),
            ),
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
        if record.stream
            && let Some(ttft) = record.tfft_ms
        {
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
        let provider = crate::types::provider::InferenceProvider::from_str(
            &record.provider,
        )
        .unwrap_or(crate::types::provider::InferenceProvider::OpenAI);
        self.health.record_attempt(
            &provider,
            &crate::config::credentials::ProviderCredentialId::new(
                &record.credential,
            ),
            record.outcome,
            record.status_code,
        );
    }

    pub fn record_client_request(&self, had_failover: bool) {
        self.runtime.record_client_request(had_failover);
    }

    pub fn record_repeat_429_violation(&self) {
        self.repeat_429_violations.add(1, &[]);
        self.runtime.record_repeat_429_violation();
    }

    #[must_use]
    pub fn snapshot(
        &self,
        provider: Option<&str>,
        credential: Option<&str>,
    ) -> ProviderStatsSnapshot {
        self.runtime.snapshot(provider, credential)
    }

    #[must_use]
    pub fn snapshot_with_credentials(
        &self,
        credentials: &crate::config::credentials::CredentialRegistry,
        provider: Option<&str>,
        credential: Option<&str>,
    ) -> ProviderStatsSnapshot {
        let mut snap = self.snapshot(provider, credential);
        merge_idle_credentials(&mut snap, credentials, provider, credential);
        merge_routing_health(&mut snap, &self.health);
        snap
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

    fn record_repeat_429_violation(&self) {
        let mut routing = self.routing.lock().expect("routing totals");
        routing.repeat_429_violations =
            routing.repeat_429_violations.saturating_add(1);
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
            CallOutcome::SemanticError => {
                entry.calls.semantic_error =
                    entry.calls.semantic_error.saturating_add(1);
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
        if record.stream
            && let Some(ttft) = record.tfft_ms
        {
            entry.tfft_sum_ms = entry.tfft_sum_ms.saturating_add(ttft as u64);
            entry.tfft_count = entry.tfft_count.saturating_add(1);
        }
        if let Some(generation_ms) = generation_ms {
            entry.generation_reservoir.record(generation_ms);
        }
        let now = Utc::now();
        entry.last_call_at = Some(now);
        entry.last_status_code = Some(record.status_code);
        if record.status_code >= 400
            || matches!(record.outcome, CallOutcome::SemanticError)
        {
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
            version: env!("CARGO_PKG_VERSION"),
            started_at,
            started_at_utc: started_at,
            started_at_server_time: started_at
                .with_timezone(&Local)
                .to_rfc3339(),
            started_at_server_timezone: started_at
                .with_timezone(&Local)
                .to_rfc3339(),
            uptime_seconds,
            providers: rows,
            quota: Vec::new(),
            routing: RoutingSnapshot {
                client_requests: routing.client_requests,
                requests_with_failover: routing.requests_with_failover,
                failover_rate,
                repeat_429_violations: routing.repeat_429_violations,
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
                semantic_error: self.calls.semantic_error,
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
            status: None,
            routing_health: None,
            quota_profile: None,
            next_available_at: None,
            blocked_reason: None,
            models: None,
        }
    }
}

fn merge_idle_credentials(
    snapshot: &mut ProviderStatsSnapshot,
    credentials: &crate::config::credentials::CredentialRegistry,
    provider: Option<&str>,
    credential: Option<&str>,
) {
    use std::collections::HashSet;

    let existing: HashSet<(String, String)> = snapshot
        .providers
        .iter()
        .map(|row| (row.provider.clone(), row.credential.clone()))
        .collect();
    for cred in credentials.all() {
        let prov = cred.provider.to_string();
        if provider.is_some_and(|p| p != prov) {
            continue;
        }
        let cred_id = cred.id.to_string();
        if credential.is_some_and(|c| c != cred_id) {
            continue;
        }
        if existing.contains(&(prov.clone(), cred_id.clone())) {
            continue;
        }
        snapshot.providers.push(idle_provider_row(prov, cred_id));
    }
    snapshot
        .providers
        .sort_by(|a, b| a.provider.cmp(&b.provider));
}

fn idle_provider_row(provider: String, credential: String) -> ProviderStatsRow {
    ProviderStatsRow {
        provider,
        credential,
        calls: ProviderCallSnapshot {
            attempts: 0,
            success: 0,
            success_degraded: 0,
            semantic_error: 0,
            client_error: 0,
            server_error: 0,
        },
        status_codes: std::collections::BTreeMap::new(),
        tokens: TokenSnapshot {
            input: 0,
            output: 0,
            cached: 0,
            reasoning: 0,
            estimated_input: 0,
            estimated_output: 0,
        },
        latency: LatencySnapshot {
            avg_duration_ms: None,
            avg_tfft_ms: None,
            avg_generation_ms_per_output_token: None,
            p50_generation_ms_per_output_token: None,
            p95_generation_ms_per_output_token: None,
        },
        last_call_at: None,
        last_error_at: None,
        last_status_code: None,
        status: Some("idle".to_string()),
        routing_health: None,
        quota_profile: None,
        next_available_at: None,
        blocked_reason: None,
        models: None,
    }
}

fn merge_routing_health(
    snapshot: &mut ProviderStatsSnapshot,
    health: &crate::router::budget_aware::CredentialHealthRegistry,
) {
    use std::time::Instant;

    let now = Instant::now();
    for row in &mut snapshot.providers {
        let provider =
            crate::types::provider::InferenceProvider::from_str(&row.provider)
                .expect("infallible provider parse");
        let credential = crate::config::credentials::ProviderCredentialId::new(
            &row.credential,
        );
        row.routing_health =
            Some(health.routing_health_snapshot(&provider, &credential, now));
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
            repeat_429_violations: self.repeat_429_violations,
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
    let tfft = if record.stream {
        record.tfft_ms.unwrap_or(0.0)
    } else {
        0.0
    };
    let gen_ms = record.duration_ms - tfft;
    if gen_ms <= 0.0 {
        return None;
    }
    Some(gen_ms / output as f64)
}

fn base_attrs(record: &AttemptRecord) -> Vec<opentelemetry::KeyValue> {
    let mut attrs = vec![
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
    ];
    if let Some(agent_name) = record.agent_name.as_deref() {
        attrs.push(opentelemetry::KeyValue::new(
            "agent_name",
            agent_name.to_string(),
        ));
    }
    attrs
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
            ttfb: record.tfft_ms,
            ttft: record.stream.then_some(record.tfft_ms).flatten(),
            generation_per_output_token: generation_ms,
        },
        routing: RoutingBlock {
            attempts: record.upstream_attempts.max(1),
            failover: record.upstream_attempts > 1,
        },
    }
}

use super::usage_json::{LatencyBlock, RoutingBlock, UsageBlock};

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_record(stream: bool, tfft_ms: Option<f64>) -> AttemptRecord {
        AttemptRecord {
            provider: "gemini".to_string(),
            credential: "gemini-free".to_string(),
            model: "gemini/gemini-3.1-flash-lite".to_string(),
            router_id: "autodefault".to_string(),
            attempt_index: 1,
            upstream_attempts: 2,
            status_code: 200,
            stream,
            request_kind: "router",
            duration_ms: 2283.0,
            tfft_ms,
            usage: TokenUsage {
                input: Some(904),
                output: Some(43),
                total: Some(947),
                ..TokenUsage::default()
            },
            usage_source: UsageSource::Reported,
            outcome: CallOutcome::Success,
            overload: false,
            agent_name: None,
        }
    }

    #[test]
    fn usage_header_reports_ttfb_not_ttft_for_non_stream() {
        let record = sample_record(false, Some(400.0));
        let usage = build_usage_header(
            &record,
            generation_ms_per_output_token(&record),
        );
        assert_eq!(usage.latency_ms.ttfb, Some(400.0));
        assert_eq!(usage.latency_ms.ttft, None);
        assert!((usage.latency_ms.total - 2283.0).abs() < f64::EPSILON);
        assert_eq!(
            usage.latency_ms.generation_per_output_token,
            Some(2283.0 / 43.0)
        );
    }

    #[test]
    fn usage_header_includes_ttft_for_stream_when_present() {
        let record = sample_record(true, Some(120.0));
        let usage = build_usage_header(
            &record,
            generation_ms_per_output_token(&record),
        );
        assert_eq!(usage.latency_ms.ttfb, Some(120.0));
        assert_eq!(usage.latency_ms.ttft, Some(120.0));
        assert_eq!(
            usage.latency_ms.generation_per_output_token,
            Some((2283.0 - 120.0) / 43.0)
        );
    }
}
