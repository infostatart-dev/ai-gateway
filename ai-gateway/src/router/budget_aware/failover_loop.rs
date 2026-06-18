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
    candidates: Vec<BudgetCandidate>,
    requirements: RequestRequirements,
    routing_intent: Option<crate::router::intent::RoutingIntent>,
) -> Result<Response, ApiError> {
    let mut failed_credentials = HashSet::<ProviderCredentialId>::new();
    let mut failed_models = HashSet::<ModelCooldownKey>::new();
    let mut route_trace = trace::RouteTrace::new(candidates.len());

    for (index, candidate) in candidates.iter().enumerate() {
        if !candidate_available(candidate, &failed_credentials, &failed_models)
        {
            route_trace.record_skipped(1);
            continue;
        }

        let has_next_candidate = candidates[index + 1..].iter().any(|next| {
            candidate_available(next, &failed_credentials, &failed_models)
        });
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
        let mut req =
            Request::from_parts(parts.clone(), Body::from(body_bytes.clone()));
        let attempt_index = route_trace.attempts().saturating_sub(1);
        let attempt_ctx = crate::types::extensions::UpstreamAttemptContext {
            attempt_index,
            upstream_attempts: route_trace.attempts(),
            credential: candidate.credential_id.to_string(),
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
                })
                .await?
            {
                return Ok(finish_success(
                    &this,
                    &mut route_trace,
                    candidate,
                    response,
                    status,
                    routing_intent,
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
    Err(ApiError::Internal(InternalError::ProviderNotFound))
}

fn finish_success(
    this: &BudgetAwareRouter,
    route_trace: &mut trace::RouteTrace,
    candidate: &BudgetCandidate,
    mut response: Response,
    status: http::StatusCode,
    routing_intent: Option<crate::router::intent::RoutingIntent>,
) -> Response {
    attach_routed_identity(
        &mut response,
        &candidate.credential_id,
        &candidate.capability.model,
    );
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

#[allow(clippy::too_many_arguments)]
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
) -> usize {
    let status = response.status();
    let model = candidate.capability.model.to_string();
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
    )
    .await;
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
