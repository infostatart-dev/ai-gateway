use bytes::Bytes;
use http::{HeaderName, StatusCode};

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
            GatewayProviderUsageExtension, PendingRouteTrace, ReplayRecord,
            RequestKind, UpstreamAttemptContext,
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
    pub agent_name: Option<&'a str>,
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
        agent_name: input.agent_name,
    });
    input.app_state.0.metrics.provider.record_attempt(&record);
}

pub fn attach_usage_header(
    app_state: &AppState,
    response: &mut http::Response<crate::types::body::Body>,
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
        agent_name: input.agent_name,
    });
    let generation_ms = generation_ms_per_output_token(&record);
    let usage = build_usage_header(&record, generation_ms);
    response
        .extensions_mut()
        .insert(GatewayProviderUsageExtension(usage.clone()));
    if let Some(value) = usage.to_header_value() {
        response.headers_mut().insert(
            HeaderName::from_static(
                super::usage_json::GATEWAY_PROVIDER_USAGE_HEADER,
            ),
            value,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        metrics::llm::TokenUsage,
        types::{
            body::Body, extensions::GatewayProviderUsageExtension,
            provider::InferenceProvider,
        },
    };

    #[tokio::test]
    async fn attach_usage_header_materializes_x_gateway_provider_usage() {
        let app_state = AppState::test_default().await;
        let input = DispatchMetricsInput {
            app_state: &app_state,
            provider: &InferenceProvider::OpenAI,
            credential: None,
            model: None,
            router_id: None,
            attempt: None,
            status: StatusCode::OK,
            stream: false,
            request_kind: RequestKind::Router,
            duration_ms: 120.0,
            tfft_ms: None,
            reported_usage: TokenUsage {
                input: Some(10),
                output: Some(5),
                total: Some(15),
                ..TokenUsage::default()
            },
            request_body: None,
            failover_class: None,
            agent_name: None,
        };

        let mut response = http::Response::builder()
            .status(StatusCode::OK)
            .body(Body::empty())
            .unwrap();
        attach_usage_header(&app_state, &mut response, &input);

        let header_name = HeaderName::from_static(
            crate::metrics::provider::usage_json::GATEWAY_PROVIDER_USAGE_HEADER,
        );
        let header = response
            .headers()
            .get(&header_name)
            .expect("x-gateway-provider-usage must be on response headers");
        let extension = response
            .extensions()
            .get::<GatewayProviderUsageExtension>()
            .expect("usage extension must be attached");
        assert_eq!(
            header.to_str().unwrap(),
            extension.0.to_header_value().unwrap().to_str().unwrap()
        );
        let usage: serde_json::Value =
            serde_json::from_str(header.to_str().unwrap()).unwrap();
        assert_eq!(
            usage.get("provider").and_then(serde_json::Value::as_str),
            Some("openai")
        );
    }
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
        quota_scope = pending.quota_scope.as_deref().unwrap_or("none"),
        model_ladder_band = pending.model_ladder_band.as_deref().unwrap_or("none"),
        model_ladder_position = pending.model_ladder_position.map_or(0, u32::from),
        upstream_failure_kind =
            pending.upstream_failure_kind.as_deref().unwrap_or("none"),
        restricted_until = pending.restricted_until.as_deref().unwrap_or("none"),
        failover_class = pending.failover_class.as_deref().unwrap_or("none"),
        agent_name = pending.agent_name.as_deref().unwrap_or("none"),
        work_unit_id = pending.work_unit_id.as_deref().unwrap_or("none"),
        work_unit_source = pending
            .work_unit_source
            .map_or("none", |source| match source {
                crate::types::extensions::WorkUnitSource::Explicit => "explicit",
                crate::types::extensions::WorkUnitSource::HeliconeSession => {
                    "helicone-session"
                }
                crate::types::extensions::WorkUnitSource::RequestId => {
                    "request-id"
                }
                crate::types::extensions::WorkUnitSource::Generated => {
                    "generated"
                }
            }),
        planned_hops = pending.planned_hops.map_or(0, u32::from),
        plan_rebuilds = pending.plan_rebuilds.map_or(0, u32::from),
        route_memory_hit = pending.route_memory_hit.is_some_and(|v| v),
        route_memory_invalidated =
            pending.route_memory_invalidated.is_some_and(|v| v),
        "budget-aware route summary"
    );
    if let Some(replay) = build_replay_record(pending) {
        tracing::info!(replay = ?replay, "route replay record");
    }
}

#[must_use]
pub fn build_replay_record(
    pending: &PendingRouteTrace,
) -> Option<ReplayRecord> {
    let agent_name = pending.agent_name.as_deref()?;
    let snapshot = pending.replay.as_ref()?;
    Some(ReplayRecord {
        agent_name: agent_name.to_string(),
        work_unit_id: pending.work_unit_id.clone(),
        source_model: pending.source_model.clone(),
        json_schema_required: pending.json_schema_required,
        planned_hops: pending.planned_hops.unwrap_or(pending.hops),
        plan_rebuilds: pending.plan_rebuilds.unwrap_or(0),
        route_memory_hit: pending.route_memory_hit.unwrap_or(false),
        route_memory_invalidated: pending
            .route_memory_invalidated
            .unwrap_or(false),
        plan_snapshot_ts: Some(snapshot.plan_snapshot_ts.clone()),
        winner_credential: Some(snapshot.winner_credential.clone()),
        winner_model: Some(snapshot.winner_model.clone()),
        winner_score: Some(snapshot.winner.clone()),
        top_alternatives: snapshot.top_alternatives.clone(),
        quota_excluded: snapshot.quota_excluded.clone(),
    })
}
