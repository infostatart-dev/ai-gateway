use std::collections::HashSet;

use axum_core::body::Body;
use http::request::Parts;
use http_body_util::BodyExt;
use tracing::Instrument;

use super::{
    call,
    cooldown::CandidateWaitOutcome,
    failure, structured_output, trace,
    types::{BudgetAwareRouter, BudgetCandidate},
};
use crate::{
    config::credentials::ProviderCredentialId,
    error::{api::ApiError, internal::InternalError},
    router::{
        capability::RequestRequirements,
        provider_attempt::{ModelCooldownKey, is_failoverable_status},
        retry_after::ExhaustionScope,
        routed_identity::attach_routed_identity,
    },
    types::{
        provider::InferenceProvider, request::Request, response::Response,
    },
};

fn candidate_available(
    candidate: &BudgetCandidate,
    failed_credentials: &HashSet<ProviderCredentialId>,
    failed_models: &HashSet<ModelCooldownKey>,
) -> bool {
    if failed_credentials.contains(&candidate.credential_id) {
        return false;
    }
    !failed_models.contains(&ModelCooldownKey {
        credential_id: candidate.credential_id.clone(),
        model: candidate.capability.model.to_string(),
    })
}

#[allow(clippy::too_many_lines)]
pub async fn run_failover_candidates(
    this: BudgetAwareRouter,
    parts: Parts,
    body_bytes: bytes::Bytes,
    candidates: Vec<BudgetCandidate>,
    requirements: RequestRequirements,
    routing_intent: Option<crate::router::intent::RoutingIntent>,
) -> Result<Response, ApiError> {
    let plan_ctx = parts
        .extensions
        .get::<crate::types::extensions::RoutePlanContext>()
        .cloned();
    let route_span = tracing::info_span!(
        "gateway.route",
        router_id = %this.router_id,
        strategy = this.strategy,
        source_model = plan_ctx
            .as_ref()
            .and_then(|ctx| ctx.source_model.as_deref())
            .unwrap_or("none"),
        candidates = candidates.len(),
        planned_hops = plan_ctx.as_ref().map_or(0, |ctx| ctx.planned_hops),
        plan_rebuilds = 0u32,
        route_memory_hit = plan_ctx
            .as_ref()
            .is_some_and(|ctx| ctx.route_memory_hit),
        route_memory_invalidated = false,
        json_schema_required = requirements.json_schema_required,
        duration_ms = tracing::field::Empty,
        tfft_ms = tracing::field::Empty,
        generation_ms_per_output_token = tracing::field::Empty,
        input_tokens = tracing::field::Empty,
        output_tokens = tracing::field::Empty,
        usage_source = tracing::field::Empty,
        failure_stage = tracing::field::Empty,
        error_source = tracing::field::Empty,
        error_class = tracing::field::Empty,
        response_body_bytes = tracing::field::Empty,
    );
    run_failover_candidates_inner(
        this,
        parts,
        body_bytes,
        candidates,
        requirements,
        routing_intent,
        plan_ctx,
        route_span.clone(),
    )
    .instrument(route_span)
    .await
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
async fn run_failover_candidates_inner(
    this: BudgetAwareRouter,
    parts: Parts,
    body_bytes: bytes::Bytes,
    mut candidates: Vec<BudgetCandidate>,
    requirements: RequestRequirements,
    routing_intent: Option<crate::router::intent::RoutingIntent>,
    plan_ctx: Option<crate::types::extensions::RoutePlanContext>,
    route_span: tracing::Span,
) -> Result<Response, ApiError> {
    let mut exclude = std::collections::HashSet::<(String, String)>::new();
    let mut plan_rebuild_count = 0u32;
    let estimated_usage = trace::estimated_usage_from_request(
        &body_bytes,
        this.app_state.config().observability.estimate_tokens,
    );
    let mut route_trace = trace::RouteTrace::new_with_plan(
        candidates.len(),
        plan_ctx.as_ref(),
        route_span.clone(),
        estimated_usage,
    );

    'walk: loop {
        let mut failed_credentials = HashSet::<ProviderCredentialId>::new();
        let mut failed_models = HashSet::<ModelCooldownKey>::new();
        route_trace.set_plan_rebuilds(plan_rebuild_count);

        for (index, candidate) in candidates.iter().enumerate() {
            if !candidate_available(
                candidate,
                &failed_credentials,
                &failed_models,
            ) {
                route_trace.event_candidate_skipped(
                    candidate,
                    "excluded_after_failure",
                    "credential_or_model",
                    0,
                );
                route_trace.record_skipped(1);
                continue;
            }

            let has_next_candidate =
                candidates[index + 1..].iter().any(|next| {
                    candidate_available(
                        next,
                        &failed_credentials,
                        &failed_models,
                    )
                });
            let estimated_tokens =
                plan_ctx.as_ref().map_or(0, |ctx| ctx.estimated_tokens);
            let admit_verdict = this
                .admit_candidate(
                    candidate,
                    estimated_tokens,
                    std::time::Instant::now(),
                )
                .await;
            if !admit_verdict.feasible {
                tracing::debug!(
                    credential = %candidate.credential_id,
                    provider = %candidate.capability.provider,
                    model = %candidate.capability.model,
                    wait_ms = admit_verdict.next_wait.as_millis(),
                    blocked_reason = ?admit_verdict.blocked_reason,
                    "re-admit skipped infeasible hop"
                );
                route_trace.event_candidate_skipped(
                    candidate,
                    "admission",
                    format!("{:?}", admit_verdict.blocked_reason),
                    admit_verdict.next_wait.as_millis(),
                );
                route_trace.record_skipped(1);
                continue;
            }
            match this.wait_for_candidate(candidate, has_next_candidate).await {
                CandidateWaitOutcome::Ready => {}
                CandidateWaitOutcome::Skipped {
                    wait,
                    blocked_reason,
                } => {
                    route_trace.event_candidate_skipped(
                        candidate,
                        "pacing",
                        format!("{blocked_reason:?}"),
                        wait.as_millis(),
                    );
                    route_trace.record_skipped(1);
                    continue;
                }
                CandidateWaitOutcome::Waited {
                    wait,
                    blocked_reason,
                } => {
                    tracing::debug!(
                        credential = %candidate.credential_id,
                        provider = %candidate.capability.provider,
                        model = %candidate.capability.model,
                        wait_ms = wait.as_millis(),
                        blocked_reason = ?blocked_reason,
                        "candidate admission resumed after wait"
                    );
                }
            }
            let model_id = candidate.capability.model.to_string();
            if budget_probe_skips(&this, candidate, model_id.as_str()).await {
                route_trace.event_candidate_skipped(
                    candidate,
                    "budget_probe",
                    "paid_budget",
                    0,
                );
                route_trace.record_skipped(1);
                continue;
            }

            route_trace.record_attempt();
            let stream = trace::request_stream_flag(&body_bytes);
            let attempt_started = std::time::Instant::now();
            let attempt_span = tracing::info_span!(
                parent: route_trace.route_span(),
                "gateway.upstream.attempt",
                attempt_index = route_trace.attempts().saturating_sub(1),
                provider = %candidate.capability.provider,
                credential = %candidate.credential_id,
                model = %candidate.capability.model,
                tier = candidate.credential_tier.as_str(),
                admit_feasible = admit_verdict.feasible,
                stream,
                estimated_tokens,
                status_code = tracing::field::Empty,
                failover_class = tracing::field::Empty,
                upstream_failure_kind = tracing::field::Empty,
                exhaustion_scope = tracing::field::Empty,
                restricted_until = tracing::field::Empty,
                failure_stage = tracing::field::Empty,
                error_source = tracing::field::Empty,
                error_class = tracing::field::Empty,
                duration_ms = tracing::field::Empty,
                tfft_ms = tracing::field::Empty,
                generation_ms_per_output_token = tracing::field::Empty,
                input_tokens = tracing::field::Empty,
                output_tokens = tracing::field::Empty,
                usage_source = tracing::field::Empty,
                response_body_bytes = tracing::field::Empty,
            );
            let mut req = Request::from_parts(
                parts.clone(),
                Body::from(body_bytes.clone()),
            );
            let attempt_index = route_trace.attempts().saturating_sub(1);
            let attempt_ctx =
                crate::types::extensions::UpstreamAttemptContext {
                    attempt_index,
                    upstream_attempts: route_trace.attempts(),
                    credential: candidate.credential_id.to_string(),
                    admit_feasible: admit_verdict.feasible,
                };
            req.extensions_mut().insert(attempt_ctx.clone());
            if let Some(tokens) = requirements.min_context_tokens {
                req.extensions_mut().insert(
                    crate::types::extensions::GatewayPayloadEstimate(tokens),
                );
            }
            let response = call::call_candidate(candidate, req)
                .instrument(attempt_span.clone())
                .await?;
            let elapsed = attempt_started.elapsed();
            let status = response.status();
            attempt_span.record("status_code", u64::from(status.as_u16()));

            if has_next_candidate && is_failoverable_status(status) {
                let skipped = fail_over_candidate(
                    &this,
                    candidate,
                    &candidates[index + 1..],
                    response,
                    elapsed,
                    crate::metrics::router::status_class(status),
                    &mut failed_credentials,
                    &mut failed_models,
                    &attempt_ctx,
                    &mut route_trace,
                    &attempt_span,
                    plan_ctx.as_ref(),
                    &mut exclude,
                )
                .await;
                route_trace.record_skipped(skipped);
                continue;
            }

            if status.is_success() {
                if let Some(response) =
                    handle_successful_candidate(SuccessCandidateContext {
                        this: &this,
                        candidate,
                        index,
                        candidates: &candidates,
                        requirements: &requirements,
                        body_bytes: &body_bytes,
                        response,
                        elapsed,
                        failed_credentials: &mut failed_credentials,
                        failed_models: &mut failed_models,
                        route_trace: &mut route_trace,
                        attempt_ctx: &attempt_ctx,
                        attempt_span: &attempt_span,
                        plan_ctx: plan_ctx.as_ref(),
                        exclude: &mut exclude,
                    })
                    .await?
                {
                    record_route_memory_success(
                        &this,
                        plan_ctx.as_ref(),
                        candidate,
                    )
                    .await;
                    route_trace.record_terminal_attempt(
                        candidate,
                        attempt_span.clone(),
                        attempt_started,
                        stream,
                    );
                    return Ok(finish_success(
                        &this,
                        &mut route_trace,
                        candidate,
                        response,
                        status,
                        routing_intent,
                        plan_ctx.as_ref(),
                    ));
                }
                continue;
            }

            if let Some(fields) =
                record_attempt_failure_fields(status, &response, &attempt_span)
            {
                route_trace.record_failure_trace_fields(&fields);
            }
            route_trace.record_terminal_attempt(
                candidate,
                attempt_span,
                attempt_started,
                stream,
            );
            return Ok(finish_terminal(
                &this,
                &mut route_trace,
                candidate,
                response,
                status,
                elapsed,
            )
            .await);
        }

        if plan_rebuild_count == 0
            && let Some(ctx) = plan_ctx.as_ref()
        {
            let previous_candidates = candidates.len();
            plan_rebuild_count = 1;
            let plan = super::plan::plan_route_chain(
                &this,
                ctx.full_pool.clone(),
                &requirements,
                routing_intent,
                &ctx.caller,
                this.app_state.credential_health(),
                this.app_state.route_memory(),
                ctx.estimated_tokens,
                &exclude,
            )
            .await;
            route_trace.record_replan(
                previous_candidates,
                plan.chain.len(),
                plan_rebuild_count,
                exclude.len(),
                if plan.chain.is_empty() {
                    "empty"
                } else {
                    "applied"
                },
            );
            if !plan.chain.is_empty() {
                candidates = plan.chain;
                if let Some(replay) = plan.replay {
                    route_trace.set_replay(replay);
                }
                continue 'walk;
            }
        }
        route_trace.emit(
            &this.router_id,
            this.strategy,
            &trace::RouteOutcome {
                label: "exhausted",
                provider: None,
                credential: None,
                status: None,
            },
            None,
        );
        return Err(ApiError::Internal(InternalError::ProviderNotFound));
    }
}

fn finish_success(
    this: &BudgetAwareRouter,
    route_trace: &mut trace::RouteTrace,
    candidate: &BudgetCandidate,
    mut response: Response,
    status: http::StatusCode,
    routing_intent: Option<crate::router::intent::RoutingIntent>,
    plan_ctx: Option<&crate::types::extensions::RoutePlanContext>,
) -> Response {
    attach_routed_identity(
        &mut response,
        &candidate.credential_id,
        &candidate.capability.model,
    );
    if let Some(ctx) = plan_ctx {
        response.extensions_mut().insert(ctx.caller.clone());
    }
    route_trace
        .record_terminal(&this.app_state.config().provider_limits, candidate);
    if let Some(intent) = routing_intent {
        let intent_context = crate::types::extensions::RoutingIntentContext {
            intent_tier: intent.preferred_tier,
            selection_phase: super::intent_selection::selection_phase_for(
                intent, candidate,
            ),
        };
        response.extensions_mut().insert(intent_context);
        this.app_state
            .0
            .metrics
            .provider
            .record_client_request(route_trace.attempts() > 1);
        let outcome = trace::RouteOutcome {
            label: "success",
            provider: Some(&candidate.capability.provider),
            credential: Some(&candidate.credential_id),
            status: Some(status.as_u16()),
        };
        let pending = route_trace.attach_pending(
            &this.router_id,
            this.strategy,
            &outcome,
            Some(intent_context),
        );
        return trace::wrap_response_with_route_trace(response, pending);
    }
    this.app_state
        .0
        .metrics
        .provider
        .record_client_request(route_trace.attempts() > 1);
    let outcome = trace::RouteOutcome {
        label: "success",
        provider: Some(&candidate.capability.provider),
        credential: Some(&candidate.credential_id),
        status: Some(status.as_u16()),
    };
    let pending = route_trace.attach_pending(
        &this.router_id,
        this.strategy,
        &outcome,
        None,
    );
    trace::wrap_response_with_route_trace(response, pending)
}

async fn finish_terminal(
    this: &BudgetAwareRouter,
    route_trace: &mut trace::RouteTrace,
    candidate: &BudgetCandidate,
    mut response: Response,
    status: http::StatusCode,
    elapsed: std::time::Duration,
) -> Response {
    if is_failoverable_status(status) {
        response = this
            .record_failure(
                &candidate.credential_id,
                &candidate.capability.provider,
                &candidate.capability.model.to_string(),
                response,
                elapsed,
            )
            .await;
    }
    attach_routed_identity(
        &mut response,
        &candidate.credential_id,
        &candidate.capability.model,
    );
    let outcome = trace::RouteOutcome {
        label: "terminal_failure",
        provider: Some(&candidate.capability.provider),
        credential: Some(&candidate.credential_id),
        status: Some(status.as_u16()),
    };
    let pending = route_trace.attach_pending(
        &this.router_id,
        this.strategy,
        &outcome,
        None,
    );
    trace::wrap_response_with_route_trace(response, pending)
}

fn record_attempt_failure_fields(
    status: http::StatusCode,
    response: &Response,
    attempt_span: &tracing::Span,
) -> Option<trace::FailureTraceFields> {
    let upstream = response
        .extensions()
        .get::<crate::types::extensions::UpstreamFailureContext>();
    let gateway = response
        .extensions()
        .get::<crate::types::extensions::GatewayFailureContext>();
    if let Some(fields) = trace::failure_trace_fields(status, upstream, gateway)
    {
        attempt_span.record("failure_stage", fields.failure_stage);
        attempt_span.record("error_source", fields.error_source);
        attempt_span.record("error_class", fields.error_class.as_str());
        return Some(fields);
    }
    None
}

struct SuccessCandidateContext<'a> {
    this: &'a BudgetAwareRouter,
    candidate: &'a BudgetCandidate,
    index: usize,
    candidates: &'a [BudgetCandidate],
    requirements: &'a RequestRequirements,
    body_bytes: &'a bytes::Bytes,
    response: Response,
    elapsed: std::time::Duration,
    failed_credentials: &'a mut HashSet<ProviderCredentialId>,
    failed_models: &'a mut HashSet<ModelCooldownKey>,
    route_trace: &'a mut trace::RouteTrace,
    attempt_ctx: &'a crate::types::extensions::UpstreamAttemptContext,
    attempt_span: &'a tracing::Span,
    plan_ctx: Option<&'a crate::types::extensions::RoutePlanContext>,
    exclude: &'a mut std::collections::HashSet<(String, String)>,
}

async fn handle_successful_candidate(
    ctx: SuccessCandidateContext<'_>,
) -> Result<Option<Response>, ApiError> {
    let SuccessCandidateContext {
        this,
        candidate,
        index,
        candidates,
        requirements,
        body_bytes,
        response,
        elapsed,
        route_trace,
        attempt_ctx,
        attempt_span,
        ..
    } = ctx;
    let has_next = index + 1 < candidates.len();
    if requirements.json_schema_required
        && candidate.capability.supports_json_schema
        && !structured_output::request_is_stream(body_bytes)
    {
        let (response_parts, response_body) = response.into_parts();
        let response_bytes = response_body
            .collect()
            .await
            .map_err(InternalError::CollectBodyError)?
            .to_bytes();
        let mut response = Response::from_parts(
            response_parts,
            Body::from(response_bytes.clone()),
        );

        if !structured_output::structured_output_valid(
            requirements,
            &candidate.capability,
            body_bytes,
            &response_bytes,
        ) {
            mark_invalid_structured_output(&mut response);
            if has_next {
                let skipped = fail_over_candidate(
                    this,
                    candidate,
                    &candidates[index + 1..],
                    response,
                    elapsed,
                    "structured_output",
                    ctx.failed_credentials,
                    ctx.failed_models,
                    attempt_ctx,
                    route_trace,
                    attempt_span,
                    ctx.plan_ctx,
                    ctx.exclude,
                )
                .await;
                route_trace.record_skipped(skipped);
            } else {
                let failure_fields = record_attempt_failure_fields(
                    response.status(),
                    &response,
                    attempt_span,
                );
                let _ = this
                    .record_failure(
                        &candidate.credential_id,
                        &candidate.capability.provider,
                        &candidate.capability.model.to_string(),
                        response,
                        elapsed,
                    )
                    .await;
                if let Some(fields) = failure_fields {
                    route_trace.record_failure_trace_fields(&fields);
                }
                ctx.failed_credentials
                    .insert(candidate.credential_id.clone());
                tracing::warn!(
                    credential = %candidate.credential_id,
                    provider = %candidate.capability.provider,
                    model = %candidate.capability.model,
                    "provider returned invalid structured JSON on last candidate"
                );
            }
            return Ok(None);
        }
        return Ok(Some(response));
    }

    this.record_success(
        &candidate.credential_id,
        &candidate.capability.provider,
        &candidate.capability.model.to_string(),
        elapsed,
    );
    if let Some(ds) = response.extensions().get::<trace::DeepSeekWebTrace>() {
        route_trace.record_deepseek_web(*ds);
    }
    if let Some(cg) = response.extensions().get::<trace::ChatGptWebTrace>() {
        route_trace.record_chatgpt_web(*cg);
    }
    Ok(Some(response))
}

fn mark_invalid_structured_output(response: &mut Response) {
    response.extensions_mut().insert(
        crate::types::extensions::GatewayFailureContext::invalid_structured_json(),
    );
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
async fn fail_over_candidate(
    this: &BudgetAwareRouter,
    candidate: &BudgetCandidate,
    remaining_candidates: &[BudgetCandidate],
    response: Response,
    elapsed: std::time::Duration,
    reason: impl AsRef<str>,
    failed_credentials: &mut HashSet<ProviderCredentialId>,
    failed_models: &mut HashSet<ModelCooldownKey>,
    attempt: &crate::types::extensions::UpstreamAttemptContext,
    route_trace: &mut trace::RouteTrace,
    attempt_span: &tracing::Span,
    plan_ctx: Option<&crate::types::extensions::RoutePlanContext>,
    exclude: &mut std::collections::HashSet<(String, String)>,
) -> usize {
    let status = response.status();
    let model = candidate.capability.model.to_string();
    if status == http::StatusCode::TOO_MANY_REQUESTS && !attempt.admit_feasible
    {
        route_trace.record_repeat_429_violation();
        this.app_state
            .0
            .metrics
            .provider
            .record_repeat_429_violation();
    }
    let failure_ctx = response
        .extensions()
        .get::<crate::types::extensions::UpstreamFailureContext>()
        .cloned();
    let gateway_failure_ctx = response
        .extensions()
        .get::<crate::types::extensions::GatewayFailureContext>()
        .cloned();
    let failure_trace = trace::failure_trace_fields(
        status,
        failure_ctx.as_ref(),
        gateway_failure_ctx.as_ref(),
    );
    let (response, class, scope) = failure::record_classified_failure(
        this,
        &candidate.credential_id,
        &candidate.capability.provider,
        &model,
        response,
        elapsed,
        candidate.credential_tier.as_str(),
    )
    .await;
    if let Some(ctx) = plan_ctx
        && let Some(work_unit) = ctx.caller.work_unit_id.as_deref()
        && let Some(binding) = this
            .app_state
            .route_memory()
            .get(&ctx.caller.agent_name, work_unit)
            .await
        && binding.credential_id == candidate.credential_id
        && binding.model == model
    {
        this.app_state
            .route_memory()
            .invalidate(&ctx.caller.agent_name, work_unit)
            .await;
        route_trace.record_route_memory_invalidated();
    }
    exclude.insert((candidate.credential_id.to_string(), model.clone()));
    route_trace.record_failure_signal(class, failure_ctx.as_ref());
    let failover_class = format!("{class:?}");
    let upstream_failure_kind = failure_ctx
        .as_ref()
        .map_or_else(|| "none".to_string(), |ctx| format!("{:?}", ctx.kind));
    let restricted_until = failure_ctx
        .as_ref()
        .and_then(|ctx| ctx.restricted_until.map(|dt| dt.to_rfc3339()))
        .unwrap_or_else(|| "none".to_string());
    let exhaustion_scope = format!("{scope:?}");
    attempt_span.record("duration_ms", elapsed.as_secs_f64() * 1000.0);
    attempt_span.record("status_code", u64::from(status.as_u16()));
    attempt_span.record("failover_class", failover_class.as_str());
    attempt_span
        .record("upstream_failure_kind", upstream_failure_kind.as_str());
    attempt_span.record("exhaustion_scope", exhaustion_scope.as_str());
    attempt_span.record("restricted_until", restricted_until.as_str());
    if let Some(fields) = failure_trace.as_ref() {
        route_trace.record_failure_trace_fields(fields);
        attempt_span.record("failure_stage", fields.failure_stage);
        attempt_span.record("error_source", fields.error_source);
        attempt_span.record("error_class", fields.error_class.as_str());
    }
    let quota_profile = this
        .app_state
        .config()
        .provider_limits
        .quota_profile(&candidate.capability.provider);
    tracing::info!(
        credential = candidate.credential_id.as_str(),
        model = %model,
        status = status.as_u16(),
        failover_class = ?class,
        exhaustion_scope = ?scope,
        quota_profile = match quota_profile {
            crate::config::provider_limits::ProviderQuotaProfile::PerModel => {
                "model"
            }
            crate::config::provider_limits::ProviderQuotaProfile::PerSlot => {
                "slot"
            }
            crate::config::provider_limits::ProviderQuotaProfile::PerSession => {
                "session"
            }
        },
        "classified upstream failure"
    );
    crate::metrics::provider::record_upstream_attempt(
        &crate::metrics::provider::DispatchMetricsInput {
            app_state: &this.app_state,
            provider: &candidate.capability.provider,
            credential: Some(&candidate.credential_id),
            model: Some(&candidate.capability.model),
            router_id: Some(&this.router_id),
            attempt: Some(attempt),
            status,
            stream: false,
            request_kind: crate::types::extensions::RequestKind::Router,
            duration_ms: elapsed.as_secs_f64() * 1000.0,
            tfft_ms: None,
            reported_usage: crate::metrics::llm::TokenUsage::default(),
            request_body: None,
            failover_class: Some(class),
            agent_name: plan_ctx.map(|ctx| ctx.caller.agent_name.as_str()),
        },
    );
    let _ = response;
    let next_provider = next_distinct_provider(
        remaining_candidates,
        failed_credentials,
        failed_models,
    );
    route_trace.event_failover(
        &candidate.capability.provider,
        next_provider,
        status.as_u16(),
        &failover_class,
        &upstream_failure_kind,
        &exhaustion_scope,
        &restricted_until,
        failure_trace.as_ref(),
    );
    failure::record_failover_metric(
        this,
        candidate,
        next_provider,
        reason.as_ref(),
        status,
        class,
    );
    match scope {
        ExhaustionScope::Model => {
            failed_models.insert(ModelCooldownKey {
                credential_id: candidate.credential_id.clone(),
                model,
            });
            0
        }
        ExhaustionScope::Slot => {
            failed_credentials.insert(candidate.credential_id.clone());
            0
        }
        ExhaustionScope::Project => {
            failed_credentials.insert(candidate.credential_id.clone());
            skip_free_siblings_on_exhaustion(
                candidate,
                remaining_candidates,
                failed_credentials,
            )
        }
    }
}

fn skip_free_siblings_on_exhaustion(
    candidate: &BudgetCandidate,
    remaining_candidates: &[BudgetCandidate],
    failed_credentials: &mut HashSet<ProviderCredentialId>,
) -> usize {
    let mut skipped = 0;
    for sibling in remaining_candidates {
        if sibling.capability.provider == candidate.capability.provider
            && sibling.credential_budget_rank
                == candidate.credential_budget_rank
            && failed_credentials.insert(sibling.credential_id.clone())
        {
            skipped += 1;
        }
    }
    skipped
}

fn next_distinct_provider<'a>(
    candidates: &'a [BudgetCandidate],
    failed_credentials: &HashSet<ProviderCredentialId>,
    failed_models: &HashSet<ModelCooldownKey>,
) -> Option<&'a InferenceProvider> {
    candidates
        .iter()
        .find(|next| {
            candidate_available(next, failed_credentials, failed_models)
        })
        .map(|c| &c.capability.provider)
}

async fn record_route_memory_success(
    router: &BudgetAwareRouter,
    plan_ctx: Option<&crate::types::extensions::RoutePlanContext>,
    candidate: &BudgetCandidate,
) {
    let Some(ctx) = plan_ctx else {
        return;
    };
    let Some(work_unit) = ctx.caller.work_unit_id.as_deref() else {
        return;
    };
    router
        .app_state
        .route_memory()
        .record(
            &ctx.caller.agent_name,
            work_unit,
            crate::router::budget_aware::memory::RouteBinding {
                credential_id: candidate.credential_id.clone(),
                model: candidate.capability.model.to_string(),
            },
        )
        .await;
}

async fn budget_probe_skips(
    router: &BudgetAwareRouter,
    candidate: &BudgetCandidate,
    model: &str,
) -> bool {
    let skip = router
        .app_state
        .budget_probe()
        .should_skip_candidate(
            &router.app_state.config().credentials,
            &candidate.capability.provider,
            &candidate.credential_id,
            model,
        )
        .await;
    if skip {
        tracing::debug!(
            credential = %candidate.credential_id,
            provider = %candidate.capability.provider,
            model = %candidate.capability.model,
            "skipping candidate with exhausted paid budget"
        );
    }
    skip
}
