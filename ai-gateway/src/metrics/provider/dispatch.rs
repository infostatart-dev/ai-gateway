use bytes::Bytes;
use http::StatusCode;

use super::{
    RecordAttemptInput, build_attempt_record, build_usage_header,
    runtime::generation_ms_per_output_token,
};
use crate::{
    app_state::AppState,
    config::credentials::ProviderCredentialId,
    metrics::llm::TokenUsage,
    router::retry_after::FailoverClass,
    types::{
        extensions::{
            GatewayProviderUsageExtension, PendingRouteTrace, RequestKind,
            UpstreamAttemptContext,
        },
        model_id::ModelId,
        provider::InferenceProvider,
        router::RouterId,
    },
};

pub struct DispatchMetricsInput<'a> {
    pub app_state: &'a AppState,
    pub provider: &'a InferenceProvider,
    pub credential: Option<&'a ProviderCredentialId>,
    pub model: Option<&'a ModelId>,
    pub router_id: Option<&'a RouterId>,
    pub attempt: Option<&'a UpstreamAttemptContext>,
    pub status: StatusCode,
    pub stream: bool,
    pub request_kind: RequestKind,
    pub duration_ms: f64,
    pub tfft_ms: Option<f64>,
    pub reported_usage: TokenUsage,
    pub request_body: Option<&'a Bytes>,
    pub failover_class: Option<FailoverClass>,
}

pub fn record_upstream_attempt(input: &DispatchMetricsInput<'_>) {
    let credential = input
        .credential
        .map(ProviderCredentialId::as_str)
        .or_else(|| input.attempt.map(|a| a.credential.as_str()))
        .unwrap_or("default");
    let record = build_attempt_record(&RecordAttemptInput {
        provider: input.provider,
        credential,
        model: input.model,
        router_id: input.router_id,
        attempt: input.attempt,
        status: input.status,
        stream: input.stream,
        request_kind: input.request_kind,
        duration_ms: input.duration_ms,
        tfft_ms: input.tfft_ms,
        reported_usage: input.reported_usage,
        request_body: input.request_body,
        estimate_tokens: input.app_state.config().observability.estimate_tokens,
        failover_class: input.failover_class,
    });
    input.app_state.0.metrics.provider.record_attempt(&record);
}

pub fn attach_usage_header(
    app_state: &AppState,
    extensions: &mut http::Extensions,
    input: &DispatchMetricsInput<'_>,
) {
    if !app_state.config().observability.response_headers.enabled {
        return;
    }
    let credential = input
        .credential
        .map(ProviderCredentialId::as_str)
        .or_else(|| input.attempt.map(|a| a.credential.as_str()))
        .unwrap_or("default");
    let record = build_attempt_record(&RecordAttemptInput {
        provider: input.provider,
        credential,
        model: input.model,
        router_id: input.router_id,
        attempt: input.attempt,
        status: input.status,
        stream: input.stream,
        request_kind: input.request_kind,
        duration_ms: input.duration_ms,
        tfft_ms: input.tfft_ms,
        reported_usage: input.reported_usage,
        request_body: input.request_body,
        estimate_tokens: app_state.config().observability.estimate_tokens,
        failover_class: input.failover_class,
    });
    let generation_ms = generation_ms_per_output_token(&record);
    extensions.insert(GatewayProviderUsageExtension(build_usage_header(
        &record,
        generation_ms,
    )));
}

pub fn emit_pending_route_trace(
    pending: &PendingRouteTrace,
    generation_ms_per_output_token: Option<f64>,
    usage_source: Option<&str>,
) {
    let provider = pending
        .terminal_provider
        .as_ref()
        .map_or_else(|| "none".to_string(), ToString::to_string);
    let credential = pending.terminal_credential.as_deref().unwrap_or("none");
    tracing::info!(
        router_id = %pending.router_id,
        strategy = pending.strategy,
        outcome = pending.outcome_label,
        hops = pending.hops,
        upstream_attempts = pending.hops,
        candidates = pending.candidates,
        skipped = pending.skipped,
        terminal_provider = provider,
        terminal_credential = credential,
        terminal_status = pending.terminal_status.map_or(0, u32::from),
        terminal_outcome = pending.outcome_label,
        generation_ms_per_output_token = generation_ms_per_output_token,
        usage_source = usage_source.unwrap_or("none"),
        deepseek_web_turns = pending.deepseek_web.map_or(0, |d| d.turns),
        deepseek_web_upload_parts =
            pending.deepseek_web.map_or(0, |d| d.upload_parts),
        deepseek_web_pow_cache_hits =
            pending.deepseek_web.map_or(0, |d| d.pow_cache_hits),
        chatgpt_web_turns = pending.chatgpt_web.map_or(0, |c| c.turns),
        chatgpt_web_upload_parts =
            pending.chatgpt_web.map_or(0, |c| c.upload_parts),
        routing_intent_tier = pending
            .intent_tier
            .map_or("none", crate::router::intent::IntentTier::as_str),
        routing_selection_phase = pending
            .selection_phase
            .map_or("none", crate::router::intent::SelectionPhase::as_str),
        "budget-aware route summary"
    );
}
