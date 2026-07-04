//! Per-request routing trace summary and backend-neutral route spans.

use std::{pin::Pin, time::Instant};

use bytes::{Bytes, BytesMut};
use futures::{Stream, StreamExt, stream};
use tracing::{Level, Span};

use super::{
    structured_output,
    trace_context::{TerminalRouteContext, terminal_route_context},
};
use crate::{
    config::{
        credentials::ProviderCredentialId,
        provider_limits::ProviderLimitCatalog,
    },
    metrics::llm::TokenUsage,
    router::{
        budget_aware::types::BudgetCandidate,
        token_estimate::{PayloadBudgetConfig, estimate_from_value},
    },
    types::{
        body::Body,
        extensions::{PendingRouteTrace, RouteTraceFinalizeContext},
        provider::InferenceProvider,
        response::Response,
        router::RouterId,
    },
};

/// `DeepSeek` Web multi-turn execution stats attached to dispatch responses.
#[derive(Debug, Clone, Copy, Default)]
pub struct DeepSeekWebTrace {
    pub turns: u32,
    pub upload_parts: u32,
    pub pow_cache_hits: u32,
}

/// `ChatGPT` Web multi-turn execution stats attached to dispatch responses.
#[derive(Debug, Clone, Copy, Default)]
pub struct ChatGptWebTrace {
    pub turns: u32,
    pub upload_parts: u32,
    pub pow_cache_hits: u32,
}

/// The terminal upstream a request settled on (or none).
pub(super) struct RouteOutcome<'a> {
    pub label: &'static str,
    pub provider: Option<&'a InferenceProvider>,
    pub credential: Option<&'a ProviderCredentialId>,
    pub status: Option<u16>,
}

pub(super) struct FailureTraceFields {
    pub failure_stage: &'static str,
    pub error_source: &'static str,
    pub error_class: String,
}

pub(super) fn failure_trace_fields(
    status: http::StatusCode,
    upstream: Option<&crate::types::extensions::UpstreamFailureContext>,
    gateway: Option<&crate::types::extensions::GatewayFailureContext>,
) -> Option<FailureTraceFields> {
    if let Some(ctx) = upstream {
        return Some(FailureTraceFields {
            failure_stage: "upstream",
            error_source: "upstream_provider",
            error_class: format!("{:?}", ctx.kind),
        });
    }
    if let Some(ctx) = gateway {
        return Some(FailureTraceFields {
            failure_stage: ctx.failure_stage,
            error_source: ctx.error_source,
            error_class: ctx.error_class.clone(),
        });
    }
    if status.is_client_error() || status.is_server_error() {
        return Some(FailureTraceFields {
            failure_stage: "upstream",
            error_source: "upstream_provider",
            error_class: format!("http_{}", status.as_u16()),
        });
    }
    None
}

pub(super) struct RouteTrace {
    started: Instant,
    route_span: Span,
    candidates: usize,
    attempts: u32,
    skipped: usize,
    deepseek_web: Option<DeepSeekWebTrace>,
    chatgpt_web: Option<ChatGptWebTrace>,
    upstream_failure_kind: Option<String>,
    restricted_until: Option<String>,
    failover_class: Option<String>,
    failure_stage: Option<String>,
    error_source: Option<String>,
    error_class: Option<String>,
    terminal: TerminalRouteContext,
    agent_name: Option<String>,
    work_unit_id: Option<String>,
    work_unit_source: Option<crate::types::extensions::WorkUnitSource>,
    planned_hops: Option<u32>,
    plan_rebuilds: u32,
    route_memory_hit: Option<bool>,
    route_memory_invalidated: bool,
    source_model: Option<String>,
    json_schema_required: bool,
    replay: Option<crate::types::extensions::PlanReplaySnapshot>,
    repeat_429_violations: u32,
    terminal_attempt_span: Option<Span>,
    terminal_attempt_started: Option<Instant>,
    terminal_model: Option<String>,
    terminal_stream: bool,
    terminal_failure_stage: Option<String>,
    terminal_error_source: Option<String>,
    terminal_error_class: Option<String>,
    estimated_usage: TokenUsage,
}

impl RouteTrace {
    pub(super) fn new_with_plan(
        candidates: usize,
        plan: Option<&crate::types::extensions::RoutePlanContext>,
        route_span: Span,
        estimated_usage: TokenUsage,
    ) -> Self {
        Self {
            started: Instant::now(),
            route_span,
            candidates,
            attempts: 0,
            skipped: 0,
            deepseek_web: None,
            chatgpt_web: None,
            upstream_failure_kind: None,
            restricted_until: None,
            failover_class: None,
            failure_stage: None,
            error_source: None,
            error_class: None,
            terminal: TerminalRouteContext::default(),
            agent_name: plan.map(|p| p.caller.agent_name.clone()),
            work_unit_id: plan.and_then(|p| p.caller.work_unit_id.clone()),
            work_unit_source: plan.map(|p| p.caller.work_unit_source),
            planned_hops: plan.map(|p| p.planned_hops),
            plan_rebuilds: 0,
            route_memory_hit: plan.map(|p| p.route_memory_hit),
            route_memory_invalidated: false,
            source_model: plan.and_then(|p| p.source_model.clone()),
            json_schema_required: plan.is_some_and(|p| p.json_schema_required),
            replay: plan.and_then(|p| p.replay.clone()),
            repeat_429_violations: 0,
            terminal_attempt_span: None,
            terminal_attempt_started: None,
            terminal_model: None,
            terminal_stream: false,
            terminal_failure_stage: None,
            terminal_error_source: None,
            terminal_error_class: None,
            estimated_usage,
        }
    }

    pub(super) fn set_replay(
        &mut self,
        replay: crate::types::extensions::PlanReplaySnapshot,
    ) {
        self.replay = Some(replay);
    }

    pub(super) fn set_plan_rebuilds(&mut self, count: u32) {
        self.plan_rebuilds = count;
        self.route_span.record("plan_rebuilds", count);
    }

    pub(super) fn record_route_memory_invalidated(&mut self) {
        self.route_memory_invalidated = true;
        self.route_span.record("route_memory_invalidated", true);
    }

    pub(super) fn record_terminal(
        &mut self,
        limits: &ProviderLimitCatalog,
        candidate: &BudgetCandidate,
    ) {
        self.terminal = terminal_route_context(limits, candidate);
    }

    pub(super) fn record_deepseek_web(&mut self, trace: DeepSeekWebTrace) {
        self.deepseek_web = Some(trace);
    }

    pub(super) fn record_chatgpt_web(&mut self, trace: ChatGptWebTrace) {
        self.chatgpt_web = Some(trace);
    }

    pub(super) fn route_span(&self) -> &Span {
        &self.route_span
    }

    pub(super) fn record_terminal_attempt(
        &mut self,
        candidate: &BudgetCandidate,
        attempt_span: Span,
        attempt_started: Instant,
        stream: bool,
        failure: Option<&FailureTraceFields>,
    ) {
        self.terminal_attempt_span = Some(attempt_span);
        self.terminal_attempt_started = Some(attempt_started);
        self.terminal_model = Some(candidate.capability.model.to_string());
        self.terminal_stream = stream;
        self.terminal_failure_stage =
            failure.map(|fields| fields.failure_stage.to_string());
        self.terminal_error_source =
            failure.map(|fields| fields.error_source.to_string());
        self.terminal_error_class =
            failure.map(|fields| fields.error_class.clone());
    }

    pub(super) fn record_failure_signal(
        &mut self,
        class: crate::router::retry_after::FailoverClass,
        ctx: Option<&crate::types::extensions::UpstreamFailureContext>,
    ) {
        self.failover_class = Some(format!("{class:?}"));
        if let Some(ctx) = ctx {
            self.upstream_failure_kind = Some(format!("{:?}", ctx.kind));
            self.restricted_until =
                ctx.restricted_until.map(|dt| dt.to_rfc3339());
        }
    }

    pub(super) fn record_failure_trace_fields(
        &mut self,
        fields: &FailureTraceFields,
    ) {
        self.failure_stage = Some(fields.failure_stage.to_string());
        self.error_source = Some(fields.error_source.to_string());
        self.error_class = Some(fields.error_class.clone());
        self.route_span
            .record("failure_stage", fields.failure_stage);
        self.route_span.record("error_source", fields.error_source);
        self.route_span
            .record("error_class", fields.error_class.as_str());
    }

    pub(super) fn record_attempt(&mut self) {
        self.attempts = self.attempts.saturating_add(1);
    }

    pub(super) fn record_skipped(&mut self, count: usize) {
        self.skipped = self.skipped.saturating_add(count);
    }

    pub(super) fn record_replan(
        &mut self,
        previous_candidates: usize,
        new_candidates: usize,
        plan_rebuilds: u32,
        excluded_candidates: usize,
        status: &'static str,
    ) {
        self.set_plan_rebuilds(plan_rebuilds);
        self.candidates = self.candidates.saturating_add(new_candidates);
        tracing::event!(
            parent: &self.route_span,
            Level::INFO,
            status,
            previous_candidates,
            new_candidates,
            candidates = self.candidates,
            plan_rebuilds,
            attempts = self.attempts,
            skipped = self.skipped,
            excluded_candidates,
            "gateway.route.replan"
        );
    }

    pub(super) fn record_repeat_429_violation(&mut self) {
        self.repeat_429_violations =
            self.repeat_429_violations.saturating_add(1);
    }

    pub(super) fn attempts(&self) -> u32 {
        self.attempts
    }

    pub(super) fn event_candidate_skipped(
        &self,
        candidate: &BudgetCandidate,
        reason: &'static str,
        blocked_reason: impl std::fmt::Display,
        wait_ms: u128,
    ) {
        let wait_ms = u64::try_from(wait_ms).unwrap_or(u64::MAX);
        tracing::event!(
            parent: &self.route_span,
            Level::INFO,
            reason,
            blocked_reason = %blocked_reason,
            wait_ms,
            provider = %candidate.capability.provider,
            credential = %candidate.credential_id,
            model = %candidate.capability.model,
            "gateway.candidate.skipped"
        );
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn event_failover(
        &self,
        from_provider: &InferenceProvider,
        to_provider: Option<&InferenceProvider>,
        status_code: u16,
        failover_class: &str,
        upstream_failure_kind: &str,
        exhaustion_scope: &str,
        restricted_until: &str,
        failure: Option<&FailureTraceFields>,
    ) {
        tracing::event!(
            parent: &self.route_span,
            Level::INFO,
            from_provider = %from_provider,
            to_provider = to_provider.map_or("none".to_string(), ToString::to_string),
            status_code,
            failover_class,
            upstream_failure_kind,
            exhaustion_scope,
            restricted_until,
            failure_stage = failure.map_or("none", |f| f.failure_stage),
            error_source = failure.map_or("none", |f| f.error_source),
            error_class = failure.map_or("none", |f| f.error_class.as_str()),
            "gateway.failover"
        );
    }

    pub(super) fn attach_pending(
        &self,
        router_id: &RouterId,
        strategy: &'static str,
        outcome: &RouteOutcome<'_>,
        intent_context: Option<crate::types::extensions::RoutingIntentContext>,
    ) -> PendingRouteTrace {
        PendingRouteTrace {
            router_id: router_id.clone(),
            strategy,
            hops: self.attempts,
            candidates: self.candidates,
            skipped: self.skipped,
            outcome_label: outcome.label,
            terminal_provider: outcome.provider.cloned(),
            terminal_credential: outcome
                .credential
                .map(ProviderCredentialId::to_string),
            terminal_status: outcome.status,
            deepseek_web: self.deepseek_web,
            chatgpt_web: self.chatgpt_web,
            intent_tier: intent_context.map(|c| c.intent_tier),
            selection_phase: intent_context.map(|c| c.selection_phase),
            quota_scope: self.terminal.quota_scope.clone(),
            model_ladder_band: self.terminal.model_ladder_band.clone(),
            model_ladder_position: self.terminal.model_ladder_position,
            upstream_failure_kind: self.upstream_failure_kind.clone(),
            restricted_until: self.restricted_until.clone(),
            failover_class: self.failover_class.clone(),
            failure_stage: self.failure_stage.clone(),
            error_source: self.error_source.clone(),
            error_class: self.error_class.clone(),
            agent_name: self.agent_name.clone(),
            work_unit_id: self.work_unit_id.clone(),
            work_unit_source: self.work_unit_source,
            planned_hops: self.planned_hops,
            plan_rebuilds: Some(self.plan_rebuilds),
            route_memory_hit: self.route_memory_hit,
            route_memory_invalidated: Some(self.route_memory_invalidated),
            source_model: self.source_model.clone(),
            json_schema_required: self.json_schema_required,
            estimated_usage: self.estimated_usage,
            replay: self.replay.clone(),
            finalize: Some(RouteTraceFinalizeContext {
                route_span: self.route_span.clone(),
                attempt_span: self.terminal_attempt_span.clone(),
                route_started: self.started,
                attempt_started: self.terminal_attempt_started,
                terminal_model: self.terminal_model.clone(),
                stream: self.terminal_stream,
                failure_stage: self.terminal_failure_stage.clone(),
                error_source: self.terminal_error_source.clone(),
                error_class: self.terminal_error_class.clone(),
            }),
        }
    }

    pub(super) fn emit(
        &self,
        router_id: &RouterId,
        strategy: &'static str,
        outcome: &RouteOutcome<'_>,
        intent_context: Option<crate::types::extensions::RoutingIntentContext>,
    ) {
        self.route_span.record(
            "duration_ms",
            self.started.elapsed().as_secs_f64() * 1000.0,
        );
        let usage_source =
            (!self.estimated_usage.is_empty()).then_some("estimated");
        crate::metrics::provider::emit_pending_route_trace(
            &self.attach_pending(router_id, strategy, outcome, intent_context),
            None,
            usage_source,
        );
    }
}

pub(super) fn estimated_usage_from_request(
    body: &Bytes,
    estimate_tokens: bool,
) -> TokenUsage {
    if !estimate_tokens {
        return TokenUsage::default();
    }
    serde_json::from_slice::<serde_json::Value>(body)
        .ok()
        .and_then(|value| {
            estimate_from_value(&value, PayloadBudgetConfig::default())
        })
        .map_or_else(TokenUsage::default, |estimate| {
            let input = u64::from(estimate.input_tokens);
            let output = u64::from(estimate.reserved_output);
            TokenUsage {
                input: Some(input),
                output: Some(output),
                total: Some(input.saturating_add(output)),
                ..TokenUsage::default()
            }
        })
}

pub(super) fn wrap_response_with_route_trace(
    mut response: Response,
    pending: PendingRouteTrace,
) -> Response {
    let Some(finalize) = pending.finalize.clone() else {
        response.extensions_mut().insert(pending);
        return response;
    };

    let mut extension_pending = pending.clone();
    extension_pending.finalize = None;
    response.extensions_mut().insert(extension_pending);

    let (parts, body) = response.into_parts();
    let state = RouteTraceBodyState::new(body, pending, finalize);
    Response::from_parts(
        parts,
        Body::from_stream(route_trace_body_stream(state)),
    )
}

type BoxedBodyDataStream = Pin<
    Box<dyn Stream<Item = Result<Bytes, axum_core::Error>> + Send + 'static>,
>;

const ROUTE_TRACE_USAGE_BUFFER_LIMIT: usize = 1024 * 1024;

struct RouteTraceBodyState {
    body: BoxedBodyDataStream,
    pending: PendingRouteTrace,
    finalize: RouteTraceFinalizeContext,
    usage_buffer: BytesMut,
    usage_buffer_truncated: bool,
    response_body_bytes: u64,
    tfft_ms: Option<f64>,
    finished: bool,
}

impl RouteTraceBodyState {
    fn new(
        body: Body,
        pending: PendingRouteTrace,
        finalize: RouteTraceFinalizeContext,
    ) -> Self {
        Self {
            body: Box::pin(body.into_data_stream()),
            pending,
            finalize,
            usage_buffer: BytesMut::new(),
            usage_buffer_truncated: false,
            response_body_bytes: 0,
            tfft_ms: None,
            finished: false,
        }
    }

    fn observe_chunk(&mut self, chunk: &Bytes) {
        if self.tfft_ms.is_none() {
            self.tfft_ms = self
                .finalize
                .attempt_started
                .map(|started| started.elapsed().as_secs_f64() * 1000.0);
        }
        self.response_body_bytes = self
            .response_body_bytes
            .saturating_add(u64::try_from(chunk.len()).unwrap_or(u64::MAX));
        self.observe_usage_bytes(chunk);
    }

    fn observe_usage_bytes(&mut self, chunk: &Bytes) {
        if self.finalize.stream {
            self.observe_stream_usage_bytes(chunk);
        } else {
            self.observe_json_usage_bytes(chunk);
        }
    }

    fn observe_json_usage_bytes(&mut self, chunk: &Bytes) {
        let remaining = ROUTE_TRACE_USAGE_BUFFER_LIMIT
            .saturating_sub(self.usage_buffer.len());
        if remaining == 0 {
            self.usage_buffer_truncated = true;
            return;
        }
        let take = remaining.min(chunk.len());
        self.usage_buffer.extend_from_slice(&chunk[..take]);
        if take < chunk.len() {
            self.usage_buffer_truncated = true;
        }
    }

    fn observe_stream_usage_bytes(&mut self, chunk: &Bytes) {
        if chunk.len() >= ROUTE_TRACE_USAGE_BUFFER_LIMIT {
            self.usage_buffer.clear();
            self.usage_buffer.extend_from_slice(
                &chunk[chunk.len() - ROUTE_TRACE_USAGE_BUFFER_LIMIT..],
            );
            self.usage_buffer_truncated = true;
            return;
        }

        let overflow = self
            .usage_buffer
            .len()
            .saturating_add(chunk.len())
            .saturating_sub(ROUTE_TRACE_USAGE_BUFFER_LIMIT);
        if overflow > 0 {
            let drain = overflow.min(self.usage_buffer.len());
            let _ = self.usage_buffer.split_to(drain);
            self.usage_buffer_truncated = true;
        }
        self.usage_buffer.extend_from_slice(chunk);
    }

    fn finish(&mut self) {
        if self.finished {
            return;
        }
        self.finished = true;

        let route_duration_ms =
            self.finalize.route_started.elapsed().as_secs_f64() * 1000.0;
        let duration_ms = self
            .finalize
            .attempt_started
            .map_or(route_duration_ms, |started| {
                started.elapsed().as_secs_f64() * 1000.0
            });
        let reported_usage =
            if self.finalize.stream || !self.usage_buffer_truncated {
                let body = self.usage_buffer.clone().freeze();
                crate::metrics::llm::extract_usage_from_response_body(
                    &body,
                    self.finalize.stream,
                )
            } else {
                TokenUsage::default()
            };
        let (usage, usage_source) =
            resolve_final_usage(reported_usage, self.pending.estimated_usage);
        let generation_ms = generation_ms_per_output_token(
            duration_ms,
            self.tfft_ms,
            self.finalize.stream,
            usage.output,
        );

        let route_fields = FinalSpanFields {
            duration_ms: route_duration_ms,
            tfft_ms: self.tfft_ms,
            generation_ms_per_output_token: generation_ms,
            input_tokens: usage.input,
            output_tokens: usage.output,
            usage_source,
            response_body_bytes: self.response_body_bytes,
        };
        record_final_fields(&self.finalize.route_span, route_fields);
        if let Some(attempt_span) = self.finalize.attempt_span.as_ref() {
            record_final_fields(
                attempt_span,
                FinalSpanFields {
                    duration_ms,
                    ..route_fields
                },
            );
            tracing::event!(
                parent: attempt_span,
                Level::INFO,
                duration_ms,
                tfft_ms = self.tfft_ms,
                generation_ms_per_output_token = generation_ms,
                input_tokens = usage.input.unwrap_or(0),
                output_tokens = usage.output.unwrap_or(0),
                usage_source,
                failure_stage = self
                    .finalize
                    .failure_stage
                    .as_deref()
                    .unwrap_or("none"),
                error_source = self
                    .finalize
                    .error_source
                    .as_deref()
                    .unwrap_or("none"),
                error_class = self
                    .finalize
                    .error_class
                    .as_deref()
                    .unwrap_or("none"),
                response_body_bytes = self.response_body_bytes,
                stream = self.finalize.stream,
                model = self.finalize.terminal_model.as_deref().unwrap_or("none"),
                "gateway.upstream.finalized"
            );
        }

        crate::metrics::provider::emit_pending_route_trace(
            &self.pending,
            generation_ms,
            Some(usage_source),
        );
    }
}

fn resolve_final_usage(
    reported: TokenUsage,
    estimated: TokenUsage,
) -> (TokenUsage, &'static str) {
    if !reported.is_empty() {
        (reported, "reported")
    } else if !estimated.is_empty() {
        (estimated, "estimated")
    } else {
        (TokenUsage::default(), "none")
    }
}

impl Drop for RouteTraceBodyState {
    fn drop(&mut self) {
        self.finish();
    }
}

fn route_trace_body_stream(
    state: RouteTraceBodyState,
) -> impl Stream<Item = Result<Bytes, axum_core::Error>> + Send + 'static {
    stream::unfold(state, |mut state| async move {
        match state.body.next().await {
            Some(Ok(chunk)) => {
                state.observe_chunk(&chunk);
                Some((Ok(chunk), state))
            }
            Some(Err(error)) => {
                state.finish();
                Some((Err(error), state))
            }
            None => {
                state.finish();
                None
            }
        }
    })
    .fuse()
}

#[derive(Clone, Copy)]
struct FinalSpanFields<'a> {
    duration_ms: f64,
    tfft_ms: Option<f64>,
    generation_ms_per_output_token: Option<f64>,
    input_tokens: Option<u64>,
    output_tokens: Option<u64>,
    usage_source: &'a str,
    response_body_bytes: u64,
}

fn record_final_fields(span: &Span, fields: FinalSpanFields<'_>) {
    span.record("duration_ms", fields.duration_ms);
    if let Some(value) = fields.tfft_ms {
        span.record("tfft_ms", value);
    }
    if let Some(value) = fields.generation_ms_per_output_token {
        span.record("generation_ms_per_output_token", value);
    }
    span.record("input_tokens", fields.input_tokens.unwrap_or(0));
    span.record("output_tokens", fields.output_tokens.unwrap_or(0));
    span.record("usage_source", fields.usage_source);
    span.record("response_body_bytes", fields.response_body_bytes);
}

#[allow(clippy::cast_precision_loss)]
fn generation_ms_per_output_token(
    duration_ms: f64,
    tfft_ms: Option<f64>,
    stream: bool,
    output_tokens: Option<u64>,
) -> Option<f64> {
    let output_tokens = output_tokens?;
    if output_tokens == 0 {
        return None;
    }
    let first_token_ms = if stream { tfft_ms.unwrap_or(0.0) } else { 0.0 };
    let generation_ms = duration_ms - first_token_ms;
    if generation_ms <= 0.0 {
        return None;
    }
    Some(generation_ms / output_tokens as f64)
}

pub(super) fn request_stream_flag(body: &Bytes) -> bool {
    structured_output::request_is_stream(body)
}

#[cfg(test)]
mod tests {
    use compact_str::CompactString;
    use http::StatusCode;
    use http_body_util::BodyExt as _;

    use super::*;
    use crate::types::router::RouterId;

    fn pending_with_finalize() -> PendingRouteTrace {
        let route_span = tracing::info_span!(
            "test.route",
            duration_ms = tracing::field::Empty,
            tfft_ms = tracing::field::Empty,
            generation_ms_per_output_token = tracing::field::Empty,
            input_tokens = tracing::field::Empty,
            output_tokens = tracing::field::Empty,
            usage_source = tracing::field::Empty,
            response_body_bytes = tracing::field::Empty,
        );
        let attempt_span = tracing::info_span!(
            "test.attempt",
            duration_ms = tracing::field::Empty,
            tfft_ms = tracing::field::Empty,
            generation_ms_per_output_token = tracing::field::Empty,
            input_tokens = tracing::field::Empty,
            output_tokens = tracing::field::Empty,
            usage_source = tracing::field::Empty,
            response_body_bytes = tracing::field::Empty,
        );
        PendingRouteTrace {
            router_id: RouterId::Named(CompactString::new("autodefault")),
            strategy: "budget-aware-capability-after",
            hops: 1,
            candidates: 1,
            skipped: 0,
            outcome_label: "success",
            terminal_provider: None,
            terminal_credential: None,
            terminal_status: Some(200),
            deepseek_web: None,
            chatgpt_web: None,
            intent_tier: None,
            selection_phase: None,
            quota_scope: None,
            model_ladder_band: None,
            model_ladder_position: None,
            upstream_failure_kind: None,
            restricted_until: None,
            failover_class: None,
            failure_stage: None,
            error_source: None,
            error_class: None,
            agent_name: None,
            work_unit_id: None,
            work_unit_source: None,
            planned_hops: Some(1),
            plan_rebuilds: Some(0),
            route_memory_hit: Some(false),
            route_memory_invalidated: Some(false),
            source_model: None,
            json_schema_required: false,
            estimated_usage: TokenUsage::default(),
            replay: None,
            finalize: Some(RouteTraceFinalizeContext {
                route_span,
                attempt_span: Some(attempt_span),
                route_started: Instant::now(),
                attempt_started: Some(Instant::now()),
                terminal_model: Some("test-model".to_string()),
                stream: false,
                failure_stage: None,
                error_source: None,
                error_class: None,
            }),
        }
    }

    #[tokio::test]
    async fn route_trace_body_wrapper_preserves_body_and_extension() {
        let body = Bytes::from(
            r#"{"usage":{"prompt_tokens":3,"completion_tokens":2,"total_tokens":5}}"#,
        );
        let response = http::Response::builder()
            .status(StatusCode::OK)
            .body(Body::from(body.clone()))
            .unwrap();

        let wrapped =
            wrap_response_with_route_trace(response, pending_with_finalize());
        let extension = wrapped
            .extensions()
            .get::<PendingRouteTrace>()
            .expect("pending route trace extension");
        assert!(extension.finalize.is_none());

        let collected = wrapped.into_body().collect().await.unwrap().to_bytes();
        assert_eq!(collected, body);
    }

    #[test]
    fn route_trace_replan_preserves_accumulated_attempts_and_skips() {
        let mut trace = RouteTrace::new_with_plan(
            1,
            None,
            Span::none(),
            TokenUsage::default(),
        );
        trace.record_attempt();
        trace.record_skipped(2);
        trace.record_replan(1, 2, 1, 1, "applied");

        let outcome = RouteOutcome {
            label: "success",
            provider: None,
            credential: None,
            status: Some(200),
        };
        let pending = trace.attach_pending(
            &RouterId::Named(CompactString::new("autodefault")),
            "budget-aware-capability-after",
            &outcome,
            None,
        );

        assert_eq!(pending.hops, 1);
        assert_eq!(pending.skipped, 2);
        assert_eq!(pending.candidates, 3);
        assert_eq!(pending.plan_rebuilds, Some(1));
        assert_eq!(pending.outcome_label, "success");
    }

    #[test]
    fn terminal_success_finalize_does_not_inherit_prior_failover_error() {
        let mut trace = RouteTrace::new_with_plan(
            1,
            None,
            Span::none(),
            TokenUsage::default(),
        );
        let prior_failure = FailureTraceFields {
            failure_stage: "structured_output",
            error_source: "gateway",
            error_class: "invalid_structured_json".to_string(),
        };
        trace.record_failure_trace_fields(&prior_failure);
        trace.terminal_attempt_span = Some(Span::none());
        trace.terminal_attempt_started = Some(Instant::now());
        trace.terminal_model = Some("winner-model".to_string());
        trace.terminal_stream = false;

        let outcome = RouteOutcome {
            label: "success",
            provider: None,
            credential: None,
            status: Some(200),
        };
        let pending = trace.attach_pending(
            &RouterId::Named(CompactString::new("autodefault")),
            "budget-aware-capability-after",
            &outcome,
            None,
        );
        let finalize = pending.finalize.as_ref().expect("finalize context");

        assert_eq!(pending.failure_stage.as_deref(), Some("structured_output"));
        assert_eq!(
            pending.error_class.as_deref(),
            Some("invalid_structured_json")
        );
        assert_eq!(finalize.failure_stage, None);
        assert_eq!(finalize.error_source, None);
        assert_eq!(finalize.error_class, None);
    }

    #[test]
    fn terminal_failure_finalize_keeps_terminal_error() {
        let mut trace = RouteTrace::new_with_plan(
            1,
            None,
            Span::none(),
            TokenUsage::default(),
        );
        let terminal_failure = FailureTraceFields {
            failure_stage: "upstream",
            error_source: "upstream_provider",
            error_class: "http_429".to_string(),
        };
        trace.terminal_attempt_span = Some(Span::none());
        trace.terminal_attempt_started = Some(Instant::now());
        trace.terminal_model = Some("failed-model".to_string());
        trace.terminal_stream = false;
        trace.terminal_failure_stage =
            Some(terminal_failure.failure_stage.to_string());
        trace.terminal_error_source =
            Some(terminal_failure.error_source.to_string());
        trace.terminal_error_class = Some(terminal_failure.error_class);

        let outcome = RouteOutcome {
            label: "terminal_failure",
            provider: None,
            credential: None,
            status: Some(429),
        };
        let pending = trace.attach_pending(
            &RouterId::Named(CompactString::new("autodefault")),
            "budget-aware-capability-after",
            &outcome,
            None,
        );
        let finalize = pending.finalize.as_ref().expect("finalize context");

        assert_eq!(finalize.failure_stage.as_deref(), Some("upstream"));
        assert_eq!(finalize.error_source.as_deref(), Some("upstream_provider"));
        assert_eq!(finalize.error_class.as_deref(), Some("http_429"));
    }

    #[test]
    fn route_trace_body_state_limits_usage_buffer() {
        let pending = pending_with_finalize();
        let finalize = pending.finalize.clone().unwrap();
        let mut state =
            RouteTraceBodyState::new(Body::empty(), pending, finalize);
        let chunk =
            Bytes::from(vec![b'a'; ROUTE_TRACE_USAGE_BUFFER_LIMIT + 128]);

        state.observe_chunk(&chunk);

        assert_eq!(
            state.response_body_bytes,
            u64::try_from(chunk.len()).unwrap()
        );
        assert_eq!(state.usage_buffer.len(), ROUTE_TRACE_USAGE_BUFFER_LIMIT);
        assert!(state.usage_buffer_truncated);
    }

    #[test]
    fn final_usage_falls_back_to_estimated_tokens() {
        let estimated = TokenUsage {
            input: Some(8),
            output: Some(3),
            total: Some(11),
            ..TokenUsage::default()
        };

        let (usage, source) =
            resolve_final_usage(TokenUsage::default(), estimated);

        assert_eq!(source, "estimated");
        assert_eq!(usage.input, Some(8));
        assert_eq!(usage.output, Some(3));
    }

    #[test]
    fn gateway_failure_context_marks_local_mapper_failures() {
        let gateway =
            crate::types::extensions::GatewayFailureContext::from_error_metric(
                "InternalError:MapperError:NoModelMapping".to_string(),
            );
        let fields = failure_trace_fields(
            StatusCode::INTERNAL_SERVER_ERROR,
            None,
            Some(&gateway),
        )
        .expect("failure fields");

        assert_eq!(fields.failure_stage, "mapper");
        assert_eq!(fields.error_source, "gateway");
        assert_eq!(
            fields.error_class,
            "InternalError:MapperError:NoModelMapping"
        );
    }

    #[test]
    fn structured_output_gateway_failure_marks_invalid_json() {
        let gateway =
            crate::types::extensions::GatewayFailureContext::invalid_structured_json();
        let fields = failure_trace_fields(StatusCode::OK, None, Some(&gateway))
            .expect("failure fields");

        assert_eq!(fields.failure_stage, "structured_output");
        assert_eq!(fields.error_source, "gateway");
        assert_eq!(fields.error_class, "invalid_structured_json");
    }
}
