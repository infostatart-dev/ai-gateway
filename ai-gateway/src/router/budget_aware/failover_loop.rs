use std::collections::HashSet;

use axum_core::{body::Body, response::IntoResponse};
use http::request::Parts;
use http_body_util::BodyExt;
use tracing::{Instrument, info_span};

use super::{
    RouteBinding, call,
    cooldown::CandidateWaitOutcome,
    failure, structured_output, trace,
    types::{BudgetAwareRouter, BudgetCandidate},
};
use crate::{
    config::credentials::ProviderCredentialId,
    error::{
        api::ApiError,
        internal::InternalError,
        invalid_req::{InvalidRequestError, TooManyRequestsError},
    },
    metrics::{
        llm::TokenUsage,
        provider::{RecordAttemptInput, attempt::CallOutcome},
    },
    router::{
        capability::RequestRequirements,
        provider_attempt::{ModelCooldownKey, is_failoverable_status},
        retry_after::{ExhaustionScope, FailoverClass},
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
    let caller = plan_ctx.as_ref().map(|ctx| &ctx.caller);
    let work_unit_source =
        caller.map_or("none", |ctx| match ctx.work_unit_source {
            crate::types::extensions::WorkUnitSource::Explicit => "explicit",
            crate::types::extensions::WorkUnitSource::HeliconeSession => {
                "helicone-session"
            }
            crate::types::extensions::WorkUnitSource::RequestId => "request-id",
            crate::types::extensions::WorkUnitSource::Generated => "generated",
        });
    let client_access = parts
        .extensions
        .get::<crate::types::extensions::ClientAccessContext>();
    let route_span = tracing::info_span!(
        "gateway.route",
        router_id = %this.router_id,
        strategy = this.strategy,
        agent_name = caller.map_or("none", |ctx| ctx.agent_name.as_str()),
        work_unit_id = caller
            .and_then(|ctx| ctx.work_unit_id.as_deref())
            .unwrap_or("none"),
        work_unit_source,
        client_subject_id = client_access
            .map_or("none", |ctx| ctx.subject_id.as_str()),
        client_key_id = client_access.map_or("none", |ctx| ctx.key_id.as_str()),
        client_plan_id = client_access
            .map_or("none", |ctx| ctx.plan_id.as_str()),
        source_model = plan_ctx
            .as_ref()
            .and_then(|ctx| ctx.source_model.as_deref())
            .unwrap_or("none"),
        candidates = candidates.len(),
        planned_hops = plan_ctx.as_ref().map_or(0, |ctx| ctx.planned_hops),
        plan_rebuilds = tracing::field::Empty,
        route_memory_hit = tracing::field::Empty,
        route_memory_invalidated = tracing::field::Empty,
        route_memory_hit_binding = tracing::field::Empty,
        route_memory_penalized_binding = tracing::field::Empty,
        route_memory_recorded_binding = tracing::field::Empty,
        route_memory_policy = tracing::field::Empty,
        json_schema_required = requirements.json_schema_required,
        attempts_total = tracing::field::Empty,
        failover_count = tracing::field::Empty,
        failed_attempts_total = tracing::field::Empty,
        attempt_statuses = tracing::field::Empty,
        attempt_error_classes = tracing::field::Empty,
        last_failover_class = tracing::field::Empty,
        last_failover_error_class = tracing::field::Empty,
        last_failed_provider = tracing::field::Empty,
        last_failed_credential = tracing::field::Empty,
        last_failed_model = tracing::field::Empty,
        duration_ms = tracing::field::Empty,
        tfft_ms = tracing::field::Empty,
        generation_ms_per_output_token = tracing::field::Empty,
        input_tokens = tracing::field::Empty,
        output_tokens = tracing::field::Empty,
        usage_source = tracing::field::Empty,
        terminal_provider = tracing::field::Empty,
        terminal_credential = tracing::field::Empty,
        terminal_model = tracing::field::Empty,
        terminal_status = tracing::field::Empty,
        terminal_outcome = tracing::field::Empty,
        terminal_error_class = tracing::field::Empty,
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
    let mut exhausted_retry_after: Option<std::time::Duration> = None;
    let mut terminal_credential_restriction: Option<TerminalFailure> = None;
    let mut terminal_no_status_failure: Option<TerminalNoStatusFailure> = None;
    let mut saw_non_credential_restriction_failure = false;
    let replan_budget = plan_ctx
        .as_ref()
        .map_or(0, |ctx| ctx.full_pool.len().saturating_add(1));
    let mut seen_plan_signatures =
        HashSet::<Vec<(String, String, String)>>::new();
    seen_plan_signatures.insert(route_plan_signature(&candidates));
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
                remember_exhausted_wait(
                    &mut exhausted_retry_after,
                    admit_verdict.next_wait,
                );
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
                    remember_exhausted_wait(&mut exhausted_retry_after, wait);
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
                remember_exhausted_wait(
                    &mut exhausted_retry_after,
                    std::time::Duration::from_secs(1),
                );
                route_trace.event_candidate_skipped(
                    candidate,
                    "budget_probe",
                    "paid_budget",
                    0,
                );
                route_trace.record_skipped(1);
                continue;
            }
            let _route_lease = match this
                .try_acquire_route_lease(candidate, has_next_candidate)
            {
                Ok(lease) => lease,
                Err(snapshot) => {
                    remember_exhausted_wait(
                        &mut exhausted_retry_after,
                        std::time::Duration::from_secs(1),
                    );
                    route_trace.event_candidate_skipped(
                        candidate,
                        "route_lease",
                        format!(
                            "in_flight active={} limit={}",
                            snapshot.active, snapshot.limit
                        ),
                        0,
                    );
                    route_trace.record_skipped(1);
                    continue;
                }
            };

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
                structured_output_normalization = tracing::field::Empty,
                duration_ms = tracing::field::Empty,
                tfft_ms = tracing::field::Empty,
                generation_ms_per_output_token = tracing::field::Empty,
                input_tokens = tracing::field::Empty,
                output_tokens = tracing::field::Empty,
                usage_source = tracing::field::Empty,
                response_body_bytes = tracing::field::Empty,
                attempt_timeout_ms = tracing::field::Empty,
            );
            route_trace.record_attempt_identity(
                candidate,
                attempt_span.clone(),
                attempt_started,
                stream,
            );
            let mut req = Request::from_parts(
                parts.clone(),
                Body::from(body_bytes.clone()),
            );
            let provider_attempt_policy =
                provider_attempt_policy(&this, candidate);
            req.extensions_mut().insert(provider_attempt_policy);
            let provider_metrics_deferred =
                structured_output_validation_required(
                    &requirements,
                    candidate,
                    &body_bytes,
                );
            if provider_metrics_deferred {
                req.extensions_mut().insert(
                    crate::types::extensions::DeferredProviderAttemptMetrics,
                );
            }
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
            let call_result =
                call_candidate_with_timeout(&this, candidate, req)
                    .instrument(attempt_span.clone())
                    .await;
            let response = match call_result {
                CandidateCallResult::Response(response) => response,
                CandidateCallResult::TimedOut { timeout } => {
                    let elapsed = attempt_started.elapsed();
                    let decision = fail_over_timeout_candidate(
                        &this,
                        candidate,
                        &candidates[index + 1..],
                        elapsed,
                        timeout,
                        attempt_started,
                        stream,
                        &failed_credentials,
                        &mut failed_models,
                        &mut route_trace,
                        &attempt_span,
                        plan_ctx.as_ref(),
                        &mut exclude,
                    )
                    .await;
                    terminal_no_status_failure = Some(decision.terminal);
                    terminal_credential_restriction = None;
                    saw_non_credential_restriction_failure = true;
                    if !candidates[index + 1..].iter().any(|next| {
                        candidate_available(
                            next,
                            &failed_credentials,
                            &failed_models,
                        )
                    }) {
                        remember_exhausted_wait(
                            &mut exhausted_retry_after,
                            std::time::Duration::from_secs(1),
                        );
                    }
                    continue;
                }
                CandidateCallResult::Failed { error } => {
                    let elapsed = attempt_started.elapsed();
                    let decision = fail_over_call_error_candidate(
                        &this,
                        candidate,
                        &candidates[index + 1..],
                        elapsed,
                        error,
                        attempt_started,
                        stream,
                        &failed_credentials,
                        &mut failed_models,
                        &mut route_trace,
                        &attempt_span,
                        plan_ctx.as_ref(),
                        &mut exclude,
                    )
                    .await;
                    terminal_no_status_failure = Some(decision.terminal);
                    terminal_credential_restriction = None;
                    saw_non_credential_restriction_failure = true;
                    if !candidates[index + 1..].iter().any(|next| {
                        candidate_available(
                            next,
                            &failed_credentials,
                            &failed_models,
                        )
                    }) {
                        remember_exhausted_wait(
                            &mut exhausted_retry_after,
                            std::time::Duration::from_secs(1),
                        );
                    }
                    continue;
                }
            };
            let elapsed = attempt_started.elapsed();
            let status = response.status();
            attempt_span.record("status_code", u64::from(status.as_u16()));

            if is_failoverable_status(status) {
                terminal_no_status_failure = None;
                let decision = fail_over_candidate(
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
                    &body_bytes,
                    provider_metrics_deferred,
                    None,
                )
                .await;
                route_trace.record_skipped(decision.skipped);
                if let Some(terminal) = decision.terminal {
                    terminal_credential_restriction = Some(terminal);
                }
                if decision.class != FailoverClass::CredentialRestricted {
                    terminal_credential_restriction = None;
                    saw_non_credential_restriction_failure = true;
                }
                if !candidates[index + 1..].iter().any(|next| {
                    candidate_available(
                        next,
                        &failed_credentials,
                        &failed_models,
                    )
                }) {
                    remember_exhausted_wait(
                        &mut exhausted_retry_after,
                        std::time::Duration::from_secs(1),
                    );
                }
                continue;
            }

            if status.is_success() {
                match handle_successful_candidate(SuccessCandidateContext {
                    this: &this,
                    parts: &parts,
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
                    CandidateSuccessOutcome::Continue => continue,
                    CandidateSuccessOutcome::Success {
                        response,
                        memory_policy,
                        terminal_attempt,
                    } => {
                        record_route_memory_success(
                            &this,
                            plan_ctx.as_ref(),
                            &mut route_trace,
                            candidate,
                            memory_policy,
                        )
                        .await;
                        let terminal_attempt_span =
                            terminal_attempt.as_ref().map_or_else(
                                || attempt_span.clone(),
                                |attempt| attempt.span.clone(),
                            );
                        let terminal_attempt_started = terminal_attempt
                            .as_ref()
                            .map_or(attempt_started, |attempt| attempt.started);
                        route_trace.record_terminal_attempt(
                            candidate,
                            terminal_attempt_span,
                            terminal_attempt_started,
                            stream,
                            None,
                        );
                        let status = response.status();
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
                    CandidateSuccessOutcome::TerminalFailure {
                        response,
                        status,
                        failure,
                    } => {
                        route_trace.record_terminal_attempt(
                            candidate,
                            attempt_span.clone(),
                            attempt_started,
                            stream,
                            failure.as_ref(),
                        );
                        return Ok(finish_terminal_response(
                            &this,
                            &mut route_trace,
                            candidate,
                            response,
                            status,
                        ));
                    }
                }
            }

            let failure_fields =
                record_attempt_failure_fields(status, &response, &attempt_span);
            if let Some(fields) = failure_fields.as_ref() {
                route_trace
                    .record_failed_attempt(candidate, status, None, fields);
            }
            route_trace.record_terminal_attempt(
                candidate,
                attempt_span,
                attempt_started,
                stream,
                failure_fields.as_ref(),
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

        if usize::try_from(plan_rebuild_count).unwrap_or(usize::MAX)
            < replan_budget
            && let Some(ctx) = plan_ctx.as_ref()
        {
            let previous_candidates = candidates.len();
            let previous_signature = route_plan_signature(&candidates);
            plan_rebuild_count = plan_rebuild_count.saturating_add(1);
            let stream_mode = if ctx.stream {
                super::RouteStreamMode::Streaming
            } else {
                super::RouteStreamMode::NonStreaming
            };
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
                ctx.source_model.as_deref(),
                stream_mode,
            )
            .await;
            let next_signature = route_plan_signature(&plan.chain);
            let repeated_plan =
                !seen_plan_signatures.insert(next_signature.clone());
            let replan_status = if plan.chain.is_empty() {
                "empty"
            } else if repeated_plan || next_signature == previous_signature {
                "unchanged"
            } else {
                "applied"
            };
            route_trace.record_replan(
                previous_candidates,
                plan.chain.len(),
                plan_rebuild_count,
                exclude.len(),
                replan_status,
                plan.replay.as_ref(),
            );
            if replan_status == "applied" {
                terminal_no_status_failure = None;
                route_trace.set_route_memory_hit_binding(
                    plan.route_memory_hit_binding.as_ref(),
                );
                candidates = plan.chain;
                if let Some(replay) = plan.replay {
                    route_trace.set_replay(replay);
                }
                terminal_credential_restriction = None;
                saw_non_credential_restriction_failure = false;
                continue 'walk;
            }
        }
        if let Some(terminal) = terminal_no_status_failure {
            return Ok(finish_no_status_terminal(
                &this,
                &mut route_trace,
                terminal,
            ));
        }
        route_trace.emit(
            &this.router_id,
            this.strategy,
            &trace::RouteOutcome {
                label: "exhausted",
                provider: None,
                credential: None,
                status: exhausted_retry_after.map(|_| 429),
            },
            None,
        );
        if let Some(retry_after) = exhausted_retry_after {
            if !saw_non_credential_restriction_failure
                && let Some(terminal) = terminal_credential_restriction
            {
                return Ok(finish_classified_terminal(
                    &this,
                    &mut route_trace,
                    terminal,
                ));
            }
            return Ok(route_exhausted_response(retry_after));
        }
        return Err(ApiError::Internal(InternalError::ProviderNotFound));
    }
}

fn route_plan_signature(
    candidates: &[BudgetCandidate],
) -> Vec<(String, String, String)> {
    candidates
        .iter()
        .map(|candidate| {
            (
                candidate.capability.provider.to_string(),
                candidate.credential_id.to_string(),
                candidate.capability.model.to_string(),
            )
        })
        .collect()
}

struct TerminalFailure {
    candidate: BudgetCandidate,
    response: Response,
    status: http::StatusCode,
}

struct TerminalNoStatusFailure {
    candidate: BudgetCandidate,
    outcome_label: &'static str,
    response: Response,
}

enum CandidateCallResult {
    Response(Response),
    TimedOut { timeout: std::time::Duration },
    Failed { error: ApiError },
}

async fn call_candidate_with_timeout(
    router: &BudgetAwareRouter,
    candidate: &BudgetCandidate,
    req: Request,
) -> CandidateCallResult {
    let timeout = router.app_state.config().provider_limits.attempt_timeout(
        &candidate.capability.provider,
        candidate.credential_tier.as_str(),
        &candidate.capability.model.to_string(),
    );
    let Some(timeout) = timeout else {
        return match call::call_candidate(candidate, req).await {
            Ok(response) => CandidateCallResult::Response(response),
            Err(error) => CandidateCallResult::Failed { error },
        };
    };
    match tokio::time::timeout(timeout, call::call_candidate(candidate, req))
        .await
    {
        Ok(Ok(response)) => CandidateCallResult::Response(response),
        Ok(Err(error)) => CandidateCallResult::Failed { error },
        Err(_) => CandidateCallResult::TimedOut { timeout },
    }
}

fn remember_exhausted_wait(
    exhausted_retry_after: &mut Option<std::time::Duration>,
    wait: std::time::Duration,
) {
    let wait = if wait.is_zero() {
        std::time::Duration::from_secs(1)
    } else {
        wait
    };
    if exhausted_retry_after.is_none_or(|current| wait < current) {
        *exhausted_retry_after = Some(wait);
    }
}

pub(crate) fn route_exhausted_response(
    retry_after: std::time::Duration,
) -> Response {
    InvalidRequestError::TooManyRequests(TooManyRequestsError {
        ratelimit_limit: 1,
        ratelimit_remaining: 0,
        retry_after: retry_after.as_secs().max(1),
    })
    .into_response()
}

fn upstream_timeout_response(timeout: std::time::Duration) -> Response {
    let body = serde_json::json!({
        "error": {
            "type": "upstream_timeout",
            "message": "upstream attempt timed out",
            "timeout_ms": timeout.as_millis(),
        }
    });
    http::Response::builder()
        .status(http::StatusCode::GATEWAY_TIMEOUT)
        .header(http::header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_string()))
        .expect("valid upstream timeout response")
}

fn upstream_incomplete_response(error: ApiError) -> Response {
    let mut response = error.into_response();
    response.extensions_mut().insert(
        crate::types::extensions::GatewayFailureContext {
            failure_stage: "transport",
            error_source: "upstream_transport",
            error_class: "upstream_incomplete".to_string(),
        },
    );
    response
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
    finish_terminal_response(this, route_trace, candidate, response, status)
}

fn finish_terminal_response(
    this: &BudgetAwareRouter,
    route_trace: &mut trace::RouteTrace,
    candidate: &BudgetCandidate,
    mut response: Response,
    status: http::StatusCode,
) -> Response {
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

fn finish_classified_terminal(
    this: &BudgetAwareRouter,
    route_trace: &mut trace::RouteTrace,
    terminal: TerminalFailure,
) -> Response {
    let TerminalFailure {
        candidate,
        mut response,
        status,
    } = terminal;
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

fn finish_no_status_terminal(
    this: &BudgetAwareRouter,
    route_trace: &mut trace::RouteTrace,
    terminal: TerminalNoStatusFailure,
) -> Response {
    let TerminalNoStatusFailure {
        candidate,
        outcome_label,
        mut response,
    } = terminal;
    attach_routed_identity(
        &mut response,
        &candidate.credential_id,
        &candidate.capability.model,
    );
    let outcome = trace::RouteOutcome {
        label: outcome_label,
        provider: Some(&candidate.capability.provider),
        credential: Some(&candidate.credential_id),
        status: None,
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
    parts: &'a Parts,
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

enum CandidateSuccessOutcome {
    Continue,
    Success {
        response: Response,
        memory_policy: RouteMemorySuccessPolicy,
        terminal_attempt: Option<TerminalAttemptTrace>,
    },
    TerminalFailure {
        response: Response,
        status: http::StatusCode,
        failure: Option<trace::FailureTraceFields>,
    },
}

struct TerminalAttemptTrace {
    span: tracing::Span,
    started: std::time::Instant,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RouteMemorySuccessPolicy {
    Full,
    Degraded,
}

fn provider_attempt_policy(
    router: &BudgetAwareRouter,
    candidate: &BudgetCandidate,
) -> crate::types::extensions::ProviderAttemptPolicy {
    crate::types::extensions::ProviderAttemptPolicy {
        slow_success_threshold: router
            .app_state
            .config()
            .provider_limits
            .slow_success_threshold(
                &candidate.capability.provider,
                candidate.credential_tier.as_str(),
                &candidate.capability.model.to_string(),
            ),
    }
}

fn route_memory_policy_for_success(
    router: &BudgetAwareRouter,
    candidate: &BudgetCandidate,
    elapsed: std::time::Duration,
) -> RouteMemorySuccessPolicy {
    let Some(threshold) =
        provider_attempt_policy(router, candidate).slow_success_threshold
    else {
        return RouteMemorySuccessPolicy::Full;
    };
    if elapsed > threshold {
        RouteMemorySuccessPolicy::Degraded
    } else {
        RouteMemorySuccessPolicy::Full
    }
}

fn outcome_for_success_policy(policy: RouteMemorySuccessPolicy) -> CallOutcome {
    match policy {
        RouteMemorySuccessPolicy::Full => CallOutcome::Success,
        RouteMemorySuccessPolicy::Degraded => CallOutcome::SuccessDegraded,
    }
}

fn record_candidate_success(
    router: &BudgetAwareRouter,
    candidate: &BudgetCandidate,
    elapsed: std::time::Duration,
    policy: RouteMemorySuccessPolicy,
) {
    match policy {
        RouteMemorySuccessPolicy::Full => router.record_success(
            &candidate.credential_id,
            &candidate.capability.provider,
            &candidate.capability.model.to_string(),
            elapsed,
        ),
        RouteMemorySuccessPolicy::Degraded => router.record_success_degraded(
            &candidate.credential_id,
            &candidate.capability.provider,
            &candidate.capability.model.to_string(),
            elapsed,
        ),
    }
}

async fn handle_successful_candidate(
    ctx: SuccessCandidateContext<'_>,
) -> Result<CandidateSuccessOutcome, ApiError> {
    let has_next = ctx.index + 1 < ctx.candidates.len();
    if structured_output_validation_required(
        ctx.requirements,
        ctx.candidate,
        ctx.body_bytes,
    ) {
        return handle_structured_output_candidate(ctx, has_next).await;
    }

    let memory_policy =
        route_memory_policy_for_success(ctx.this, ctx.candidate, ctx.elapsed);
    record_candidate_success(
        ctx.this,
        ctx.candidate,
        ctx.elapsed,
        memory_policy,
    );
    if let Some(ds) = ctx.response.extensions().get::<trace::DeepSeekWebTrace>()
    {
        ctx.route_trace.record_deepseek_web(*ds);
    }
    if let Some(cg) = ctx.response.extensions().get::<trace::ChatGptWebTrace>()
    {
        ctx.route_trace.record_chatgpt_web(*cg);
    }
    Ok(CandidateSuccessOutcome::Success {
        response: ctx.response,
        memory_policy,
        terminal_attempt: None,
    })
}

fn structured_output_validation_required(
    requirements: &RequestRequirements,
    candidate: &BudgetCandidate,
    body_bytes: &bytes::Bytes,
) -> bool {
    requirements.json_schema_required
        && candidate.capability.supports_json_schema
        && !structured_output::request_is_stream(body_bytes)
}

async fn handle_structured_output_candidate(
    ctx: SuccessCandidateContext<'_>,
    has_next: bool,
) -> Result<CandidateSuccessOutcome, ApiError> {
    let response = ctx.response;
    let (response, response_bytes) = normalized_structured_response(
        response,
        ctx.candidate,
        ctx.attempt_span,
    )
    .await?;
    let validation = validate_structured_candidate(
        ctx.requirements,
        ctx.candidate,
        ctx.body_bytes,
        &response_bytes,
    );
    if validation.is_valid_or_skipped() {
        let memory_policy = route_memory_policy_for_success(
            ctx.this,
            ctx.candidate,
            ctx.elapsed,
        );
        return Ok(finish_structured_output_success(StructuredOutputSuccess {
            this: ctx.this,
            candidate: ctx.candidate,
            response,
            elapsed: ctx.elapsed,
            request_body: ctx.body_bytes,
            response_body: &response_bytes,
            attempt_ctx: ctx.attempt_ctx,
            agent_name: deferred_agent_name(ctx.plan_ctx),
            outcome: outcome_for_success_policy(memory_policy),
            memory_policy,
            terminal_attempt: None,
        }));
    }

    let validation_issue = validation.issue();
    record_initial_structured_output_semantic_attempt(
        &StructuredOutputAttempt {
            this: ctx.this,
            candidate: ctx.candidate,
            attempt: ctx.attempt_ctx,
            status: response.status(),
            elapsed: ctx.elapsed,
            request_body: ctx.body_bytes,
            response_body: Some(&response_bytes),
            outcome: CallOutcome::SemanticError,
            failover_class: None,
            agent_name: deferred_agent_name(ctx.plan_ctx),
        },
        validation_issue.as_ref(),
    );
    if let Some(outcome) =
        try_structured_output_recovery(StructuredOutputRecoveryContext {
            this: ctx.this,
            parts: ctx.parts,
            candidate: ctx.candidate,
            requirements: ctx.requirements,
            request_body: ctx.body_bytes,
            response_body: &response_bytes,
            elapsed: ctx.elapsed,
            route_trace: ctx.route_trace,
            attempt_ctx: ctx.attempt_ctx,
            plan_ctx: ctx.plan_ctx,
            validation_issue,
        })
        .await?
    {
        return Ok(outcome);
    }
    handle_invalid_structured_output(
        InvalidStructuredOutputContext {
            this: ctx.this,
            candidate: ctx.candidate,
            index: ctx.index,
            candidates: ctx.candidates,
            elapsed: ctx.elapsed,
            failed_credentials: ctx.failed_credentials,
            failed_models: ctx.failed_models,
            route_trace: ctx.route_trace,
            attempt_ctx: ctx.attempt_ctx,
            attempt_span: ctx.attempt_span,
            plan_ctx: ctx.plan_ctx,
            exclude: ctx.exclude,
            request_body: ctx.body_bytes,
            response_body: &response_bytes,
            semantic_attempt_recorded: true,
        },
        response,
        validation_issue,
        has_next,
    )
    .await
}

fn validate_structured_candidate(
    requirements: &RequestRequirements,
    candidate: &BudgetCandidate,
    request_body: &bytes::Bytes,
    response_body: &bytes::Bytes,
) -> structured_output::StructuredOutputValidation {
    structured_output::validate_structured_output(
        requirements,
        &candidate.capability,
        request_body,
        response_body,
    )
}

fn record_initial_structured_output_semantic_attempt(
    input: &StructuredOutputAttempt<'_>,
    validation_issue: Option<&web_structured_output::StructuredOutputIssue>,
) {
    tracing::warn!(
        credential = %input.candidate.credential_id,
        provider = %input.candidate.capability.provider,
        model = %input.candidate.capability.model,
        phase = "initial",
        validation_issue = ?validation_issue,
        "JSON validation exception from structured output"
    );
    record_structured_output_attempt(input);
}

struct StructuredOutputRecoveryContext<'a> {
    this: &'a BudgetAwareRouter,
    parts: &'a Parts,
    candidate: &'a BudgetCandidate,
    requirements: &'a RequestRequirements,
    request_body: &'a bytes::Bytes,
    response_body: &'a bytes::Bytes,
    elapsed: std::time::Duration,
    route_trace: &'a mut trace::RouteTrace,
    attempt_ctx: &'a crate::types::extensions::UpstreamAttemptContext,
    plan_ctx: Option<&'a crate::types::extensions::RoutePlanContext>,
    validation_issue: Option<web_structured_output::StructuredOutputIssue>,
}

async fn try_structured_output_recovery(
    ctx: StructuredOutputRecoveryContext<'_>,
) -> Result<Option<CandidateSuccessOutcome>, ApiError> {
    let StructuredOutputRecoveryContext {
        this,
        parts,
        candidate,
        requirements,
        request_body,
        response_body,
        elapsed,
        route_trace,
        attempt_ctx,
        plan_ctx,
        validation_issue,
    } = ctx;
    let agent_name = deferred_agent_name(plan_ctx);

    if let Some(retry_success) =
        try_structured_output_retry(StructuredOutputRetryContext {
            this,
            candidate,
            parts,
            request_body,
            requirements,
            attempt_ctx,
            route_trace,
            plan_ctx,
            validation_issue,
        })
        .await?
    {
        return Ok(Some(finish_structured_output_success(
            StructuredOutputSuccess {
                this,
                candidate,
                response: retry_success.response,
                elapsed: retry_success.elapsed,
                request_body,
                response_body: &retry_success.response_body,
                attempt_ctx: &retry_success.attempt_ctx,
                agent_name,
                outcome: CallOutcome::SuccessDegraded,
                memory_policy: RouteMemorySuccessPolicy::Degraded,
                terminal_attempt: Some(TerminalAttemptTrace {
                    span: retry_success.span,
                    started: retry_success.started,
                }),
            },
        )));
    }

    if let Some((reflected_response, reflected_bytes)) =
        try_schema_conformance_reflector(ReflectorContext {
            this,
            candidate,
            parts,
            request_body,
            response_body,
            requirements,
            attempt_ctx,
            route_trace,
        })
        .await?
    {
        return Ok(Some(finish_structured_output_success(
            StructuredOutputSuccess {
                this,
                candidate,
                response: reflected_response,
                elapsed,
                request_body,
                response_body: &reflected_bytes,
                attempt_ctx,
                agent_name,
                outcome: CallOutcome::SuccessDegraded,
                memory_policy: RouteMemorySuccessPolicy::Degraded,
                terminal_attempt: None,
            },
        )));
    }

    Ok(None)
}

async fn normalized_structured_response(
    response: Response,
    candidate: &BudgetCandidate,
    attempt_span: &tracing::Span,
) -> Result<(Response, bytes::Bytes), ApiError> {
    let (parts, response_bytes) = collect_normalized_response_body(
        response,
        candidate,
        "initial",
        attempt_span,
    )
    .await?;
    Ok((
        Response::from_parts(parts, Body::from(response_bytes.clone())),
        response_bytes,
    ))
}

struct StructuredOutputSuccess<'a> {
    this: &'a BudgetAwareRouter,
    candidate: &'a BudgetCandidate,
    response: Response,
    elapsed: std::time::Duration,
    request_body: &'a bytes::Bytes,
    response_body: &'a bytes::Bytes,
    attempt_ctx: &'a crate::types::extensions::UpstreamAttemptContext,
    agent_name: Option<&'a str>,
    outcome: CallOutcome,
    memory_policy: RouteMemorySuccessPolicy,
    terminal_attempt: Option<TerminalAttemptTrace>,
}

fn finish_structured_output_success(
    input: StructuredOutputSuccess<'_>,
) -> CandidateSuccessOutcome {
    let StructuredOutputSuccess {
        this,
        candidate,
        response,
        elapsed,
        request_body,
        response_body,
        attempt_ctx,
        agent_name,
        outcome,
        memory_policy,
        terminal_attempt,
    } = input;
    let success_policy = match outcome {
        CallOutcome::Success => RouteMemorySuccessPolicy::Full,
        CallOutcome::SuccessDegraded => RouteMemorySuccessPolicy::Degraded,
        _ => unreachable!("structured output success used non-success outcome"),
    };
    record_candidate_success(this, candidate, elapsed, success_policy);
    record_deferred_provider_attempt(&DeferredProviderAttempt {
        this,
        candidate,
        attempt: attempt_ctx,
        status: http::StatusCode::OK,
        elapsed,
        request_body,
        response_body: Some(response_body),
        outcome,
        failover_class: None,
        agent_name,
    });
    CandidateSuccessOutcome::Success {
        response,
        memory_policy,
        terminal_attempt,
    }
}

struct DeferredProviderAttempt<'a> {
    this: &'a BudgetAwareRouter,
    candidate: &'a BudgetCandidate,
    attempt: &'a crate::types::extensions::UpstreamAttemptContext,
    status: http::StatusCode,
    elapsed: std::time::Duration,
    request_body: &'a bytes::Bytes,
    response_body: Option<&'a bytes::Bytes>,
    outcome: CallOutcome,
    failover_class: Option<FailoverClass>,
    agent_name: Option<&'a str>,
}

struct StructuredOutputAttempt<'a> {
    this: &'a BudgetAwareRouter,
    candidate: &'a BudgetCandidate,
    attempt: &'a crate::types::extensions::UpstreamAttemptContext,
    status: http::StatusCode,
    elapsed: std::time::Duration,
    request_body: &'a bytes::Bytes,
    response_body: Option<&'a bytes::Bytes>,
    outcome: CallOutcome,
    failover_class: Option<FailoverClass>,
    agent_name: Option<&'a str>,
}

fn deferred_agent_name(
    plan_ctx: Option<&crate::types::extensions::RoutePlanContext>,
) -> Option<&str> {
    plan_ctx.map(|ctx| ctx.caller.agent_name.as_str())
}

fn build_deferred_provider_attempt_record(
    input: &DeferredProviderAttempt<'_>,
) -> crate::metrics::provider::AttemptRecord {
    let reported_usage =
        input
            .response_body
            .map_or_else(TokenUsage::default, |body| {
                crate::metrics::llm::extract_usage_from_response_body(
                    body, false,
                )
            });
    crate::metrics::provider::build_attempt_record(&RecordAttemptInput {
        provider: &input.candidate.capability.provider,
        credential: input.candidate.credential_id.as_str(),
        model: Some(&input.candidate.capability.model),
        router_id: Some(&input.this.router_id),
        attempt: Some(input.attempt),
        status: input.status,
        stream: false,
        request_kind: crate::types::extensions::RequestKind::Router,
        duration_ms: input.elapsed.as_secs_f64() * 1000.0,
        tfft_ms: None,
        reported_usage,
        request_body: Some(input.request_body),
        estimate_tokens: input
            .this
            .app_state
            .config()
            .observability
            .estimate_tokens,
        failover_class: input.failover_class,
        semantic_outcome: Some(input.outcome),
        agent_name: input.agent_name,
    })
}

fn record_deferred_provider_attempt(input: &DeferredProviderAttempt<'_>) {
    let record = build_deferred_provider_attempt_record(input);
    input
        .this
        .app_state
        .0
        .metrics
        .provider
        .record_attempt(&record);
}

fn record_structured_output_attempt(input: &StructuredOutputAttempt<'_>) {
    record_deferred_provider_attempt(&DeferredProviderAttempt {
        this: input.this,
        candidate: input.candidate,
        attempt: input.attempt,
        status: input.status,
        elapsed: input.elapsed,
        request_body: input.request_body,
        response_body: input.response_body,
        outcome: input.outcome,
        failover_class: input.failover_class,
        agent_name: input.agent_name,
    });
    input
        .this
        .app_state
        .credential_health()
        .record_model_attempt(
            &input.candidate.capability.provider,
            &input.candidate.credential_id,
            &input.candidate.capability.model.to_string(),
            input.outcome,
            input.status.as_u16(),
            input.elapsed,
        );
}

fn outcome_for_failover(
    status: http::StatusCode,
    class: FailoverClass,
) -> CallOutcome {
    if status == http::StatusCode::TOO_MANY_REQUESTS {
        return CallOutcome::RateLimited;
    }
    if class == FailoverClass::Overload {
        return CallOutcome::Overload;
    }
    if status.is_server_error() {
        return CallOutcome::ServerError;
    }
    CallOutcome::ClientError
}

async fn collect_normalized_response_body(
    response: Response,
    candidate: &BudgetCandidate,
    phase: &'static str,
    span: &tracing::Span,
) -> Result<(http::response::Parts, bytes::Bytes), ApiError> {
    let (response_parts, response_body) = response.into_parts();
    let mut response_bytes = response_body
        .collect()
        .await
        .map_err(InternalError::CollectBodyError)?
        .to_bytes();
    normalize_markdown_fenced_json(candidate, phase, span, &mut response_bytes);
    Ok((response_parts, response_bytes))
}

fn normalize_markdown_fenced_json(
    candidate: &BudgetCandidate,
    phase: &'static str,
    span: &tracing::Span,
    response_bytes: &mut bytes::Bytes,
) {
    if !structured_output::markdown_fenced_json_normalizer_enabled(
        &candidate.capability.provider,
    ) {
        return;
    }
    let Some(normalized) =
        structured_output::normalize_markdown_fenced_json_response(
            response_bytes,
        )
    else {
        return;
    };

    *response_bytes = normalized;
    span.record("structured_output_normalization", "markdown_fenced_json");
    tracing::warn!(
        credential = %candidate.credential_id,
        provider = %candidate.capability.provider,
        model = %candidate.capability.model,
        phase,
        normalization = "markdown_fenced_json",
        "applied semi-heuristic structured JSON normalization"
    );
}

struct InvalidStructuredOutputContext<'a> {
    this: &'a BudgetAwareRouter,
    candidate: &'a BudgetCandidate,
    index: usize,
    candidates: &'a [BudgetCandidate],
    elapsed: std::time::Duration,
    failed_credentials: &'a mut HashSet<ProviderCredentialId>,
    failed_models: &'a mut HashSet<ModelCooldownKey>,
    route_trace: &'a mut trace::RouteTrace,
    attempt_ctx: &'a crate::types::extensions::UpstreamAttemptContext,
    attempt_span: &'a tracing::Span,
    plan_ctx: Option<&'a crate::types::extensions::RoutePlanContext>,
    exclude: &'a mut std::collections::HashSet<(String, String)>,
    request_body: &'a bytes::Bytes,
    response_body: &'a bytes::Bytes,
    semantic_attempt_recorded: bool,
}

async fn handle_invalid_structured_output(
    mut ctx: InvalidStructuredOutputContext<'_>,
    mut response: Response,
    validation_issue: Option<web_structured_output::StructuredOutputIssue>,
    has_next: bool,
) -> Result<CandidateSuccessOutcome, ApiError> {
    mark_invalid_structured_output(&mut response);
    let status = response.status();
    let gateway_failure_ctx = response
        .extensions()
        .get::<crate::types::extensions::GatewayFailureContext>()
        .cloned();
    let failure_fields = record_semantic_structured_output_failure(
        &mut ctx,
        status,
        gateway_failure_ctx,
        has_next,
    )
    .await;
    if has_next {
        return Ok(CandidateSuccessOutcome::Continue);
    }

    tracing::warn!(
        credential = %ctx.candidate.credential_id,
        provider = %ctx.candidate.capability.provider,
        model = %ctx.candidate.capability.model,
        validation_issue = ?validation_issue,
        "provider returned invalid structured JSON on last candidate"
    );
    let response = route_exhausted_response(std::time::Duration::from_secs(1));
    let status = response.status();
    Ok(CandidateSuccessOutcome::TerminalFailure {
        response,
        status,
        failure: failure_fields,
    })
}

async fn record_semantic_structured_output_failure(
    ctx: &mut InvalidStructuredOutputContext<'_>,
    status: http::StatusCode,
    gateway_failure_ctx: Option<
        crate::types::extensions::GatewayFailureContext,
    >,
    has_next: bool,
) -> Option<trace::FailureTraceFields> {
    let model = ctx.candidate.capability.model.to_string();
    record_semantic_model_health(ctx, status, &model);
    let failure_trace = record_semantic_trace_fields(
        ctx,
        status,
        gateway_failure_ctx.as_ref(),
        has_next,
    );
    penalize_semantic_route_memory(ctx, &model).await;
    mark_semantic_model_failed(ctx, model);
    let next_provider = semantic_next_provider(ctx, has_next);
    record_semantic_failover_event(
        ctx,
        status,
        next_provider.as_ref(),
        failure_trace.as_ref(),
    );
    failure_trace
}

fn record_semantic_model_health(
    ctx: &mut InvalidStructuredOutputContext<'_>,
    status: http::StatusCode,
    model: &str,
) {
    if !ctx.semantic_attempt_recorded {
        record_structured_output_attempt(&StructuredOutputAttempt {
            this: ctx.this,
            candidate: ctx.candidate,
            attempt: ctx.attempt_ctx,
            status,
            elapsed: ctx.elapsed,
            request_body: ctx.request_body,
            response_body: Some(ctx.response_body),
            outcome: CallOutcome::SemanticError,
            failover_class: None,
            agent_name: deferred_agent_name(ctx.plan_ctx),
        });
    }
    let semantic_cooldown = ctx
        .this
        .app_state
        .config()
        .provider_limits
        .cooldown_for(&ctx.candidate.capability.provider)
        .provider_error;
    ctx.this.update_failure_state_scoped(
        &ctx.candidate.credential_id,
        model,
        ExhaustionScope::Model,
        ctx.elapsed,
        semantic_cooldown,
    );
}

fn record_semantic_trace_fields(
    ctx: &mut InvalidStructuredOutputContext<'_>,
    status: http::StatusCode,
    gateway_failure_ctx: Option<
        &crate::types::extensions::GatewayFailureContext,
    >,
    has_next: bool,
) -> Option<trace::FailureTraceFields> {
    let failure_trace =
        trace::failure_trace_fields(status, None, gateway_failure_ctx);
    if let Some(fields) = failure_trace.as_ref() {
        ctx.route_trace.record_failed_attempt(
            ctx.candidate,
            status,
            has_next.then_some("semantic_error"),
            fields,
        );
        ctx.attempt_span
            .record("failure_stage", fields.failure_stage);
        ctx.attempt_span.record("error_source", fields.error_source);
        ctx.attempt_span
            .record("error_class", fields.error_class.as_str());
    }
    ctx.route_trace.record_semantic_failure_signal();
    ctx.attempt_span
        .record("duration_ms", ctx.elapsed.as_secs_f64() * 1000.0);
    ctx.attempt_span
        .record("status_code", u64::from(status.as_u16()));
    ctx.attempt_span.record("failover_class", "semantic_error");
    ctx.attempt_span.record("upstream_failure_kind", "none");
    ctx.attempt_span.record("exhaustion_scope", "model");
    ctx.attempt_span.record("restricted_until", "none");
    failure_trace
}

async fn penalize_semantic_route_memory(
    ctx: &mut InvalidStructuredOutputContext<'_>,
    model: &str,
) {
    let binding = RouteBinding {
        credential_id: ctx.candidate.credential_id.clone(),
        model: model.to_string(),
    };
    penalize_route_memory_if_present(
        ctx.this,
        ctx.route_trace,
        ctx.plan_ctx,
        &binding,
    )
    .await;
}

fn mark_semantic_model_failed(
    ctx: &mut InvalidStructuredOutputContext<'_>,
    model: String,
) {
    ctx.exclude
        .insert((ctx.candidate.credential_id.to_string(), model.clone()));
    ctx.failed_models.insert(ModelCooldownKey {
        credential_id: ctx.candidate.credential_id.clone(),
        model,
    });
}

fn semantic_next_provider(
    ctx: &InvalidStructuredOutputContext<'_>,
    has_next: bool,
) -> Option<InferenceProvider> {
    let remaining_candidates = if has_next {
        &ctx.candidates[ctx.index + 1..]
    } else {
        &[]
    };
    let next_provider = next_distinct_provider(
        remaining_candidates,
        &*ctx.failed_credentials,
        &*ctx.failed_models,
    );
    next_provider.cloned()
}

fn record_semantic_failover_event(
    ctx: &mut InvalidStructuredOutputContext<'_>,
    status: http::StatusCode,
    next_provider: Option<&InferenceProvider>,
    failure_trace: Option<&trace::FailureTraceFields>,
) {
    ctx.route_trace.event_failover(
        &ctx.candidate.capability.provider,
        next_provider,
        status.as_u16(),
        "semantic_error",
        "none",
        "model",
        "none",
        failure_trace,
    );
    failure::record_failover_metric(
        ctx.this,
        ctx.candidate,
        next_provider,
        "structured_output",
        status,
        FailoverClass::Transient,
    );
}

struct ReflectorContext<'a> {
    this: &'a BudgetAwareRouter,
    candidate: &'a BudgetCandidate,
    parts: &'a Parts,
    request_body: &'a bytes::Bytes,
    response_body: &'a bytes::Bytes,
    requirements: &'a RequestRequirements,
    attempt_ctx: &'a crate::types::extensions::UpstreamAttemptContext,
    route_trace: &'a mut trace::RouteTrace,
}

struct StructuredOutputRetryContext<'a> {
    this: &'a BudgetAwareRouter,
    candidate: &'a BudgetCandidate,
    parts: &'a Parts,
    request_body: &'a bytes::Bytes,
    requirements: &'a RequestRequirements,
    attempt_ctx: &'a crate::types::extensions::UpstreamAttemptContext,
    route_trace: &'a mut trace::RouteTrace,
    plan_ctx: Option<&'a crate::types::extensions::RoutePlanContext>,
    validation_issue: Option<web_structured_output::StructuredOutputIssue>,
}

struct StructuredOutputRetrySuccess {
    response: Response,
    response_body: bytes::Bytes,
    attempt_ctx: crate::types::extensions::UpstreamAttemptContext,
    span: tracing::Span,
    started: std::time::Instant,
    elapsed: std::time::Duration,
}

async fn try_structured_output_retry(
    ctx: StructuredOutputRetryContext<'_>,
) -> Result<Option<StructuredOutputRetrySuccess>, ApiError> {
    let Some(retry_body) =
        structured_output::build_structured_output_retry_request(
            ctx.request_body,
            ctx.validation_issue,
        )
    else {
        tracing::warn!(
            provider = %ctx.candidate.capability.provider,
            model = %ctx.candidate.capability.model,
            "structured output retry could not build repair request"
        );
        return Ok(None);
    };

    ctx.route_trace.record_attempt();
    let attempt_index = ctx.route_trace.attempts().saturating_sub(1);
    let retry_attempt_ctx = crate::types::extensions::UpstreamAttemptContext {
        attempt_index,
        upstream_attempts: ctx.route_trace.attempts(),
        credential: ctx.candidate.credential_id.to_string(),
        admit_feasible: ctx.attempt_ctx.admit_feasible,
    };

    let retry_span = info_span!(
        parent: ctx.route_trace.route_span(),
        "gateway.structured_output.retry",
        provider = %ctx.candidate.capability.provider,
        credential = %ctx.candidate.credential_id,
        model = %ctx.candidate.capability.model,
        attempt_index = retry_attempt_ctx.attempt_index,
        upstream_attempts = retry_attempt_ctx.upstream_attempts,
        status_code = tracing::field::Empty,
        failover_class = tracing::field::Empty,
        upstream_failure_kind = tracing::field::Empty,
        exhaustion_scope = tracing::field::Empty,
        restricted_until = tracing::field::Empty,
        failure_stage = tracing::field::Empty,
        error_source = tracing::field::Empty,
        error_class = tracing::field::Empty,
        valid = tracing::field::Empty,
        validation_issue = tracing::field::Empty,
        structured_output_normalization = tracing::field::Empty,
        duration_ms = tracing::field::Empty,
        attempt_timeout_ms = tracing::field::Empty,
    );
    let instrument_span = retry_span.clone();
    let retry_started = std::time::Instant::now();
    execute_structured_output_retry(
        ctx,
        retry_body,
        retry_span,
        retry_attempt_ctx,
        retry_started,
    )
    .instrument(instrument_span)
    .await
}

async fn execute_structured_output_retry(
    ctx: StructuredOutputRetryContext<'_>,
    retry_body: bytes::Bytes,
    retry_span: tracing::Span,
    retry_attempt_ctx: crate::types::extensions::UpstreamAttemptContext,
    retry_started: std::time::Instant,
) -> Result<Option<StructuredOutputRetrySuccess>, ApiError> {
    tracing::info!("structured output retry started");
    let mut retry_req =
        Request::from_parts(ctx.parts.clone(), Body::from(retry_body));
    retry_req.extensions_mut().insert(retry_attempt_ctx.clone());
    retry_req
        .extensions_mut()
        .insert(crate::types::extensions::DeferredProviderAttemptMetrics);

    let retry_response =
        match call_candidate_with_timeout(ctx.this, ctx.candidate, retry_req)
            .await
        {
            CandidateCallResult::Response(response) => response,
            CandidateCallResult::TimedOut { timeout } => {
                record_structured_retry_timeout(
                    ctx.route_trace,
                    ctx.candidate,
                    &retry_span,
                    retry_started,
                    timeout,
                );
                return Ok(None);
            }
            CandidateCallResult::Failed { error } => {
                record_structured_retry_transport_error(
                    ctx.route_trace,
                    ctx.candidate,
                    &retry_span,
                    retry_started,
                    &error,
                );
                return Ok(None);
            }
        };
    let status = retry_response.status();
    retry_span.record("status_code", u64::from(status.as_u16()));
    if !status.is_success() {
        record_structured_retry_http_failure(
            &ctx,
            &retry_attempt_ctx,
            &retry_span,
            retry_started,
            status,
        );
        return Ok(None);
    }

    let (parts, retry_bytes) = collect_normalized_response_body(
        retry_response,
        ctx.candidate,
        "retry",
        &retry_span,
    )
    .await?;
    let validation = structured_output::validate_structured_output(
        ctx.requirements,
        &ctx.candidate.capability,
        ctx.request_body,
        &retry_bytes,
    );
    retry_span.record("valid", validation.is_valid_or_skipped());
    if !validation.is_valid_or_skipped() {
        record_structured_retry_invalid_response(
            &ctx,
            &retry_attempt_ctx,
            &retry_span,
            retry_started,
            status,
            &retry_bytes,
            validation.issue(),
        );
        return Ok(None);
    }

    tracing::info!("structured output retry repaired response");
    let retry_elapsed = retry_started.elapsed();
    Ok(Some(StructuredOutputRetrySuccess {
        response: Response::from_parts(parts, Body::from(retry_bytes.clone())),
        response_body: retry_bytes,
        attempt_ctx: retry_attempt_ctx,
        span: retry_span,
        started: retry_started,
        elapsed: retry_elapsed,
    }))
}

fn record_structured_retry_http_failure(
    ctx: &StructuredOutputRetryContext<'_>,
    retry_attempt_ctx: &crate::types::extensions::UpstreamAttemptContext,
    retry_span: &tracing::Span,
    retry_started: std::time::Instant,
    status: http::StatusCode,
) {
    retry_span.record("valid", false);
    record_structured_output_attempt(&StructuredOutputAttempt {
        this: ctx.this,
        candidate: ctx.candidate,
        attempt: retry_attempt_ctx,
        status,
        elapsed: retry_started.elapsed(),
        request_body: ctx.request_body,
        response_body: None,
        outcome: outcome_for_failover(status, FailoverClass::Transient),
        failover_class: None,
        agent_name: deferred_agent_name(ctx.plan_ctx),
    });
    tracing::warn!(
        status = status.as_u16(),
        "structured output retry upstream call failed"
    );
}

fn record_structured_retry_invalid_response(
    ctx: &StructuredOutputRetryContext<'_>,
    retry_attempt_ctx: &crate::types::extensions::UpstreamAttemptContext,
    retry_span: &tracing::Span,
    retry_started: std::time::Instant,
    status: http::StatusCode,
    retry_bytes: &bytes::Bytes,
    validation_issue: Option<web_structured_output::StructuredOutputIssue>,
) {
    retry_span.record(
        "validation_issue",
        tracing::field::display(format!("{validation_issue:?}")),
    );
    tracing::warn!(
        provider = %ctx.candidate.capability.provider,
        credential = %ctx.candidate.credential_id,
        model = %ctx.candidate.capability.model,
        phase = "retry",
        validation_issue = ?validation_issue,
        "structured output retry returned invalid structured JSON"
    );
    record_structured_output_attempt(&StructuredOutputAttempt {
        this: ctx.this,
        candidate: ctx.candidate,
        attempt: retry_attempt_ctx,
        status,
        elapsed: retry_started.elapsed(),
        request_body: ctx.request_body,
        response_body: Some(retry_bytes),
        outcome: CallOutcome::SemanticError,
        failover_class: None,
        agent_name: deferred_agent_name(ctx.plan_ctx),
    });
}

fn record_structured_retry_timeout(
    route_trace: &mut trace::RouteTrace,
    candidate: &BudgetCandidate,
    retry_span: &tracing::Span,
    retry_started: std::time::Instant,
    timeout: std::time::Duration,
) {
    let fields = trace::FailureTraceFields::upstream_timeout();
    route_trace.record_failed_attempt_without_status(
        candidate,
        Some("semantic_retry_timeout"),
        &fields,
    );
    record_attempt_timeout_span(
        retry_span,
        retry_started.elapsed(),
        timeout,
        &fields,
        "semantic_retry_timeout",
    );
}

fn record_structured_retry_transport_error(
    route_trace: &mut trace::RouteTrace,
    candidate: &BudgetCandidate,
    retry_span: &tracing::Span,
    retry_started: std::time::Instant,
    error: &ApiError,
) {
    tracing::warn!(
        provider = %candidate.capability.provider,
        credential = %candidate.credential_id,
        model = %candidate.capability.model,
        error = %error,
        "structured output retry failed before HTTP status"
    );
    let fields = trace::FailureTraceFields::upstream_incomplete();
    route_trace.record_failed_attempt_without_status(
        candidate,
        Some("semantic_retry_transport_error"),
        &fields,
    );
    record_attempt_transport_error_span(
        retry_span,
        retry_started.elapsed(),
        &fields,
        "semantic_retry_transport_error",
    );
}

async fn try_schema_conformance_reflector(
    ctx: ReflectorContext<'_>,
) -> Result<Option<(Response, bytes::Bytes)>, ApiError> {
    if !structured_output::schema_conformance_reflector_enabled(
        &ctx.candidate.capability.provider,
    ) {
        return Ok(None);
    }
    let Some(repair_body) =
        structured_output::build_schema_conformance_reflection_request(
            ctx.request_body,
            ctx.response_body,
        )
    else {
        tracing::warn!(
            provider = %ctx.candidate.capability.provider,
            model = %ctx.candidate.capability.model,
            "schema conformance reflector could not build repair request"
        );
        return Ok(None);
    };

    let repair_span = info_span!(
        parent: ctx.route_trace.route_span(),
        "gateway.structured_output.reflector",
        provider = %ctx.candidate.capability.provider,
        credential = %ctx.candidate.credential_id,
        model = %ctx.candidate.capability.model,
        attempt_index = ctx.attempt_ctx.attempt_index,
        status_code = tracing::field::Empty,
        failover_class = tracing::field::Empty,
        upstream_failure_kind = tracing::field::Empty,
        exhaustion_scope = tracing::field::Empty,
        restricted_until = tracing::field::Empty,
        failure_stage = tracing::field::Empty,
        error_source = tracing::field::Empty,
        error_class = tracing::field::Empty,
        valid = tracing::field::Empty,
        validation_issue = tracing::field::Empty,
        structured_output_normalization = tracing::field::Empty,
        duration_ms = tracing::field::Empty,
        attempt_timeout_ms = tracing::field::Empty,
    );
    let instrument_span = repair_span.clone();
    execute_schema_conformance_reflector(ctx, repair_body, repair_span)
        .instrument(instrument_span)
        .await
}

async fn execute_schema_conformance_reflector(
    ctx: ReflectorContext<'_>,
    repair_body: bytes::Bytes,
    repair_span: tracing::Span,
) -> Result<Option<(Response, bytes::Bytes)>, ApiError> {
    tracing::info!("schema conformance reflector started");
    let mut repair_req =
        Request::from_parts(ctx.parts.clone(), Body::from(repair_body));
    repair_req.extensions_mut().insert(ctx.attempt_ctx.clone());
    repair_req
        .extensions_mut()
        .insert(crate::types::extensions::DeferredProviderAttemptMetrics);

    let repair_started = std::time::Instant::now();
    let repair_response =
        match call_candidate_with_timeout(ctx.this, ctx.candidate, repair_req)
            .await
        {
            CandidateCallResult::Response(response) => response,
            CandidateCallResult::TimedOut { timeout } => {
                let fields = trace::FailureTraceFields::upstream_timeout();
                record_attempt_timeout_span(
                    &repair_span,
                    repair_started.elapsed(),
                    timeout,
                    &fields,
                    "semantic_reflector_timeout",
                );
                return Ok(None);
            }
            CandidateCallResult::Failed { error } => {
                tracing::warn!(
                    provider = %ctx.candidate.capability.provider,
                    credential = %ctx.candidate.credential_id,
                    model = %ctx.candidate.capability.model,
                    error = %error,
                    "schema conformance reflector failed before HTTP status"
                );
                let fields = trace::FailureTraceFields::upstream_incomplete();
                record_attempt_transport_error_span(
                    &repair_span,
                    repair_started.elapsed(),
                    &fields,
                    "semantic_reflector_transport_error",
                );
                return Ok(None);
            }
        };
    let status = repair_response.status();
    repair_span.record("status_code", u64::from(status.as_u16()));
    if !status.is_success() {
        repair_span.record("valid", false);
        tracing::warn!(
            status = status.as_u16(),
            "schema conformance reflector upstream call failed"
        );
        return Ok(None);
    }

    let (parts, repair_bytes) = collect_normalized_response_body(
        repair_response,
        ctx.candidate,
        "reflector",
        &repair_span,
    )
    .await?;
    let validation = structured_output::validate_structured_output(
        ctx.requirements,
        &ctx.candidate.capability,
        ctx.request_body,
        &repair_bytes,
    );
    repair_span.record("valid", validation.is_valid_or_skipped());
    if !validation.is_valid_or_skipped() {
        let validation_issue = validation.issue();
        repair_span.record(
            "validation_issue",
            tracing::field::display(format!("{validation_issue:?}")),
        );
        tracing::warn!(
            provider = %ctx.candidate.capability.provider,
            credential = %ctx.candidate.credential_id,
            model = %ctx.candidate.capability.model,
            phase = "reflector",
            validation_issue = ?validation_issue,
            "schema conformance reflector returned invalid structured JSON"
        );
        return Ok(None);
    }

    tracing::info!("schema conformance reflector repaired response");
    Ok(Some((
        Response::from_parts(parts, Body::from(repair_bytes.clone())),
        repair_bytes,
    )))
}

fn mark_invalid_structured_output(response: &mut Response) {
    response.extensions_mut().insert(
        crate::types::extensions::GatewayFailureContext::invalid_structured_json(),
    );
}

fn record_attempt_timeout_span(
    attempt_span: &tracing::Span,
    elapsed: std::time::Duration,
    timeout: std::time::Duration,
    fields: &trace::FailureTraceFields,
    failover_class: &'static str,
) {
    attempt_span.record("duration_ms", elapsed.as_secs_f64() * 1000.0);
    attempt_span.record("status_code", 0u64);
    attempt_span.record("valid", false);
    attempt_span.record("failover_class", failover_class);
    attempt_span.record("upstream_failure_kind", "timeout");
    attempt_span.record("exhaustion_scope", "model");
    attempt_span.record("restricted_until", "none");
    attempt_span.record("failure_stage", fields.failure_stage);
    attempt_span.record("error_source", fields.error_source);
    attempt_span.record("error_class", fields.error_class.as_str());
    attempt_span.record(
        "attempt_timeout_ms",
        u64::try_from(timeout.as_millis()).unwrap_or(u64::MAX),
    );
}

fn record_attempt_transport_error_span(
    attempt_span: &tracing::Span,
    elapsed: std::time::Duration,
    fields: &trace::FailureTraceFields,
    failover_class: &'static str,
) {
    attempt_span.record("duration_ms", elapsed.as_secs_f64() * 1000.0);
    attempt_span.record("status_code", 0u64);
    attempt_span.record("valid", false);
    attempt_span.record("failover_class", failover_class);
    attempt_span.record("upstream_failure_kind", "transport_error");
    attempt_span.record("exhaustion_scope", "model");
    attempt_span.record("restricted_until", "none");
    attempt_span.record("failure_stage", fields.failure_stage);
    attempt_span.record("error_source", fields.error_source);
    attempt_span.record("error_class", fields.error_class.as_str());
}

async fn penalize_route_memory_if_present(
    router: &BudgetAwareRouter,
    route_trace: &mut trace::RouteTrace,
    plan_ctx: Option<&crate::types::extensions::RoutePlanContext>,
    binding: &RouteBinding,
) -> bool {
    let Some(ctx) = plan_ctx else {
        return false;
    };
    let penalized = router
        .app_state
        .route_memory()
        .penalize(&ctx.route_memory_key, binding)
        .await;
    if penalized {
        route_trace.record_route_memory_penalized(binding);
        route_trace.record_route_memory_invalidated();
    }
    penalized
}

struct NoStatusFailureRecord<'a> {
    this: &'a BudgetAwareRouter,
    candidate: &'a BudgetCandidate,
    remaining_candidates: &'a [BudgetCandidate],
    elapsed: std::time::Duration,
    attempt_started: std::time::Instant,
    stream: bool,
    failed_credentials: &'a HashSet<ProviderCredentialId>,
    failed_models: &'a mut HashSet<ModelCooldownKey>,
    route_trace: &'a mut trace::RouteTrace,
    attempt_span: &'a tracing::Span,
    plan_ctx: Option<&'a crate::types::extensions::RoutePlanContext>,
    exclude: &'a mut std::collections::HashSet<(String, String)>,
    fields: &'a trace::FailureTraceFields,
    upstream_failure_kind: &'static str,
    metric_reason: &'static str,
    metric_status: http::StatusCode,
}

async fn record_no_status_failure(mut record: NoStatusFailureRecord<'_>) {
    let model = record.candidate.capability.model.to_string();
    record_no_status_health(&record, &model);
    let cooldown = record
        .this
        .app_state
        .config()
        .provider_limits
        .cooldown_for(&record.candidate.capability.provider)
        .provider_error;
    let _ = record.this.update_failure_state_scoped(
        &record.candidate.credential_id,
        &model,
        ExhaustionScope::Model,
        record.elapsed,
        cooldown,
    );

    let binding = RouteBinding {
        credential_id: record.candidate.credential_id.clone(),
        model: model.clone(),
    };
    penalize_route_memory_if_present(
        record.this,
        record.route_trace,
        record.plan_ctx,
        &binding,
    )
    .await;

    mark_no_status_candidate_failed(&mut record, model);
    let has_available_next = no_status_has_available_next(&record);
    let next_provider = next_distinct_provider(
        record.remaining_candidates,
        record.failed_credentials,
        record.failed_models,
    )
    .cloned();
    record_no_status_trace(
        &mut record,
        has_available_next,
        next_provider.as_ref(),
    );
}

fn record_no_status_health(record: &NoStatusFailureRecord<'_>, model: &str) {
    record
        .this
        .app_state
        .credential_health()
        .record_failover_class(
            &record.candidate.capability.provider,
            &record.candidate.credential_id,
            FailoverClass::Transient,
            ExhaustionScope::Model,
        );
    record
        .this
        .app_state
        .credential_health()
        .record_model_attempt(
            &record.candidate.capability.provider,
            &record.candidate.credential_id,
            model,
            CallOutcome::ServerError,
            0,
            record.elapsed,
        );
    record
        .this
        .app_state
        .credential_health()
        .record_model_failover_class(
            &record.candidate.capability.provider,
            &record.candidate.credential_id,
            model,
            FailoverClass::Transient,
            0,
        );
}

fn mark_no_status_candidate_failed(
    record: &mut NoStatusFailureRecord<'_>,
    model: String,
) {
    record
        .exclude
        .insert((record.candidate.credential_id.to_string(), model.clone()));
    record.failed_models.insert(ModelCooldownKey {
        credential_id: record.candidate.credential_id.clone(),
        model,
    });
}

fn no_status_has_available_next(record: &NoStatusFailureRecord<'_>) -> bool {
    record.remaining_candidates.iter().any(|next| {
        candidate_available(
            next,
            record.failed_credentials,
            record.failed_models,
        )
    })
}

fn record_no_status_trace(
    record: &mut NoStatusFailureRecord<'_>,
    has_available_next: bool,
    next_provider: Option<&InferenceProvider>,
) {
    record.route_trace.record_failed_attempt_without_status(
        record.candidate,
        has_available_next.then_some("Transient"),
        record.fields,
    );
    record.route_trace.record_terminal_attempt(
        record.candidate,
        record.attempt_span.clone(),
        record.attempt_started,
        record.stream,
        Some(record.fields),
    );
    record.route_trace.event_failover(
        &record.candidate.capability.provider,
        next_provider,
        0,
        "Transient",
        record.upstream_failure_kind,
        "model",
        "none",
        Some(record.fields),
    );
    failure::record_failover_metric(
        record.this,
        record.candidate,
        next_provider,
        record.metric_reason,
        record.metric_status,
        FailoverClass::Transient,
    );
}

#[allow(clippy::too_many_arguments)]
async fn fail_over_timeout_candidate(
    this: &BudgetAwareRouter,
    candidate: &BudgetCandidate,
    remaining_candidates: &[BudgetCandidate],
    elapsed: std::time::Duration,
    timeout: std::time::Duration,
    attempt_started: std::time::Instant,
    stream: bool,
    failed_credentials: &HashSet<ProviderCredentialId>,
    failed_models: &mut HashSet<ModelCooldownKey>,
    route_trace: &mut trace::RouteTrace,
    attempt_span: &tracing::Span,
    plan_ctx: Option<&crate::types::extensions::RoutePlanContext>,
    exclude: &mut std::collections::HashSet<(String, String)>,
) -> NoStatusFailoverDecision {
    let fields = trace::FailureTraceFields::upstream_timeout();
    route_trace.record_failure_signal(FailoverClass::Transient, None);
    record_attempt_timeout_span(
        attempt_span,
        elapsed,
        timeout,
        &fields,
        "Transient",
    );
    record_no_status_failure(NoStatusFailureRecord {
        this,
        candidate,
        remaining_candidates,
        elapsed,
        attempt_started,
        stream,
        failed_credentials,
        failed_models,
        route_trace,
        attempt_span,
        plan_ctx,
        exclude,
        fields: &fields,
        upstream_failure_kind: "timeout",
        metric_reason: "upstream_timeout",
        metric_status: http::StatusCode::GATEWAY_TIMEOUT,
    })
    .await;

    NoStatusFailoverDecision {
        terminal: TerminalNoStatusFailure {
            candidate: candidate.clone(),
            outcome_label: "upstream_timeout",
            response: upstream_timeout_response(timeout),
        },
    }
}

#[allow(clippy::too_many_arguments)]
async fn fail_over_call_error_candidate(
    this: &BudgetAwareRouter,
    candidate: &BudgetCandidate,
    remaining_candidates: &[BudgetCandidate],
    elapsed: std::time::Duration,
    error: ApiError,
    attempt_started: std::time::Instant,
    stream: bool,
    failed_credentials: &HashSet<ProviderCredentialId>,
    failed_models: &mut HashSet<ModelCooldownKey>,
    route_trace: &mut trace::RouteTrace,
    attempt_span: &tracing::Span,
    plan_ctx: Option<&crate::types::extensions::RoutePlanContext>,
    exclude: &mut std::collections::HashSet<(String, String)>,
) -> NoStatusFailoverDecision {
    tracing::warn!(
        provider = %candidate.capability.provider,
        credential = %candidate.credential_id,
        model = %candidate.capability.model,
        error = %error,
        "upstream call failed before HTTP status"
    );
    let fields = trace::FailureTraceFields::upstream_incomplete();
    route_trace.record_failure_signal(FailoverClass::Transient, None);
    record_attempt_transport_error_span(
        attempt_span,
        elapsed,
        &fields,
        "Transient",
    );
    record_no_status_failure(NoStatusFailureRecord {
        this,
        candidate,
        remaining_candidates,
        elapsed,
        attempt_started,
        stream,
        failed_credentials,
        failed_models,
        route_trace,
        attempt_span,
        plan_ctx,
        exclude,
        fields: &fields,
        upstream_failure_kind: "transport_error",
        metric_reason: "upstream_incomplete",
        metric_status: http::StatusCode::BAD_GATEWAY,
    })
    .await;

    NoStatusFailoverDecision {
        terminal: TerminalNoStatusFailure {
            candidate: candidate.clone(),
            outcome_label: "upstream_incomplete",
            response: upstream_incomplete_response(error),
        },
    }
}

struct NoStatusFailoverDecision {
    terminal: TerminalNoStatusFailure,
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
    request_body: &bytes::Bytes,
    provider_metrics_deferred: bool,
    semantic_outcome: Option<CallOutcome>,
) -> FailoverDecision {
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
    if provider_metrics_deferred {
        record_deferred_provider_attempt(&DeferredProviderAttempt {
            this,
            candidate,
            attempt,
            status,
            elapsed,
            request_body,
            response_body: None,
            outcome: semantic_outcome
                .unwrap_or_else(|| outcome_for_failover(status, class)),
            failover_class: Some(class),
            agent_name: deferred_agent_name(plan_ctx),
        });
    }
    let binding = RouteBinding {
        credential_id: candidate.credential_id.clone(),
        model: model.clone(),
    };
    penalize_route_memory_if_present(this, route_trace, plan_ctx, &binding)
        .await;
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
        route_trace.record_failed_attempt(
            candidate,
            status,
            Some(failover_class.as_str()),
            fields,
        );
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
    let terminal = (class == FailoverClass::CredentialRestricted).then(|| {
        TerminalFailure {
            candidate: candidate.clone(),
            response,
            status,
        }
    });
    match scope {
        ExhaustionScope::Model => {
            failed_models.insert(ModelCooldownKey {
                credential_id: candidate.credential_id.clone(),
                model,
            });
            FailoverDecision {
                skipped: 0,
                class,
                terminal,
            }
        }
        ExhaustionScope::Slot => {
            failed_credentials.insert(candidate.credential_id.clone());
            FailoverDecision {
                skipped: 0,
                class,
                terminal,
            }
        }
        ExhaustionScope::Project => {
            failed_credentials.insert(candidate.credential_id.clone());
            let skipped = skip_free_siblings_on_exhaustion(
                candidate,
                remaining_candidates,
                failed_credentials,
            );
            FailoverDecision {
                skipped,
                class,
                terminal,
            }
        }
    }
}

struct FailoverDecision {
    skipped: usize,
    class: FailoverClass,
    terminal: Option<TerminalFailure>,
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
    route_trace: &mut trace::RouteTrace,
    candidate: &BudgetCandidate,
    policy: RouteMemorySuccessPolicy,
) {
    let Some(ctx) = plan_ctx else {
        return;
    };
    let binding = crate::router::budget_aware::memory::RouteBinding {
        credential_id: candidate.credential_id.clone(),
        model: candidate.capability.model.to_string(),
    };
    match policy {
        RouteMemorySuccessPolicy::Full => {
            route_trace.record_route_memory_recorded(&binding, "full");
            router
                .app_state
                .route_memory()
                .record(&ctx.route_memory_key, binding)
                .await;
        }
        RouteMemorySuccessPolicy::Degraded => {
            route_trace.record_route_memory_recorded(&binding, "degraded");
            router
                .app_state
                .route_memory()
                .record_degraded(&ctx.route_memory_key, binding)
                .await;
        }
    }
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

#[cfg(all(test, feature = "testing"))]
mod tests {
    use std::time::Duration;

    use bytes::Bytes;

    use super::*;
    use crate::{
        router::budget_aware::{empty_router, named_model_candidate},
        types::extensions::UpstreamAttemptContext,
    };

    #[tokio::test]
    async fn deferred_provider_attempt_preserves_agent_name() {
        let app_state = crate::app_state::AppState::test_default().await;
        let router = empty_router(&app_state);
        let candidate = named_model_candidate(
            &app_state,
            "llm7",
            "llm7-default",
            "fast",
            32_000,
        )
        .await;
        let attempt = UpstreamAttemptContext {
            attempt_index: 0,
            upstream_attempts: 1,
            credential: "llm7-default".to_string(),
            admit_feasible: true,
        };
        let request_body = Bytes::from_static(br#"{"messages":[]}"#);

        let record =
            build_deferred_provider_attempt_record(&DeferredProviderAttempt {
                this: &router,
                candidate: &candidate,
                attempt: &attempt,
                status: http::StatusCode::OK,
                elapsed: Duration::from_millis(12),
                request_body: &request_body,
                response_body: None,
                outcome: CallOutcome::Success,
                failover_class: None,
                agent_name: Some("invoker-alpha"),
            });

        assert_eq!(record.agent_name.as_deref(), Some("invoker-alpha"));
    }
}
