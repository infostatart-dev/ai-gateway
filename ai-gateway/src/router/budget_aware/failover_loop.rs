use std::collections::HashSet;

use axum_core::body::Body;
use http::request::Parts;
use http_body_util::BodyExt;

use super::{
    call, failure, structured_output, trace,
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
    mut candidates: Vec<BudgetCandidate>,
    requirements: RequestRequirements,
    routing_intent: Option<crate::router::intent::RoutingIntent>,
) -> Result<Response, ApiError> {
    let plan_ctx = parts
        .extensions
        .get::<crate::types::extensions::RoutePlanContext>()
        .cloned();
    let mut exclude = std::collections::HashSet::<(String, String)>::new();
    let mut plan_rebuild_count = 0u32;
    let mut current_replay =
        plan_ctx.as_ref().and_then(|ctx| ctx.replay.clone());

    'walk: loop {
        let mut failed_credentials = HashSet::<ProviderCredentialId>::new();
        let mut failed_models = HashSet::<ModelCooldownKey>::new();
        let mut route_trace = trace::RouteTrace::new_with_plan(
            candidates.len(),
            plan_ctx.as_ref(),
        );
        if let Some(replay) = current_replay.clone() {
            route_trace.set_replay(replay);
        }
        route_trace.set_plan_rebuilds(plan_rebuild_count);

        for (index, candidate) in candidates.iter().enumerate() {
            if !candidate_available(
                candidate,
                &failed_credentials,
                &failed_models,
            ) {
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
                route_trace.record_skipped(1);
                continue;
            }
            if !this.wait_for_candidate(candidate, has_next_candidate).await {
                route_trace.record_skipped(1);
                continue;
            }
            let model_id = candidate.capability.model.to_string();
            if budget_probe_skips(&this, candidate, model_id.as_str()).await {
                route_trace.record_skipped(1);
                continue;
            }

            route_trace.record_attempt();
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
            let start = std::time::Instant::now();
            let response = call::call_candidate(candidate, req).await?;
            let elapsed = start.elapsed();
            let status = response.status();

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
        if plan_rebuild_count == 0
            && let Some(ctx) = plan_ctx.as_ref()
        {
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
            if !plan.chain.is_empty() {
                candidates = plan.chain;
                current_replay = plan.replay;
                continue 'walk;
            }
        }
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
        response.extensions_mut().insert(route_trace.attach_pending(
            &this.router_id,
            this.strategy,
            &outcome,
            Some(intent_context),
        ));
        return response;
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
    response.extensions_mut().insert(route_trace.attach_pending(
        &this.router_id,
        this.strategy,
        &outcome,
        None,
    ));
    response
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
    route_trace.emit(
        &this.router_id,
        this.strategy,
        &trace::RouteOutcome {
            label: "terminal_failure",
            provider: Some(&candidate.capability.provider),
            credential: Some(&candidate.credential_id),
            status: Some(status.as_u16()),
        },
        None,
    );
    response
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
        let response = Response::from_parts(
            response_parts,
            Body::from(response_bytes.clone()),
        );

        if !structured_output::structured_output_valid(
            requirements,
            &candidate.capability,
            body_bytes,
            &response_bytes,
        ) {
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
                    ctx.plan_ctx,
                    ctx.exclude,
                )
                .await;
                route_trace.record_skipped(skipped);
            } else {
                let _ = this
                    .record_failure(
                        &candidate.credential_id,
                        &candidate.capability.provider,
                        &candidate.capability.model.to_string(),
                        response,
                        elapsed,
                    )
                    .await;
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
