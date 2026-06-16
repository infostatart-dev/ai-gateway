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
        provider_attempt::is_failoverable_status, retry_after::FailoverClass,
        routed_identity::attach_routed_identity,
    },
    types::{
        provider::InferenceProvider, request::Request, response::Response,
    },
};

pub(super) async fn run_failover_candidates(
    this: BudgetAwareRouter,
    parts: Parts,
    body_bytes: bytes::Bytes,
    candidates: Vec<BudgetCandidate>,
    requirements: RequestRequirements,
) -> Result<Response, ApiError> {
    let mut failed_credentials = HashSet::<ProviderCredentialId>::new();
    let mut route_trace = trace::RouteTrace::new(candidates.len());

    for (index, candidate) in candidates.iter().enumerate() {
        if failed_credentials.contains(&candidate.credential_id) {
            route_trace.record_skipped(1);
            continue;
        }

        let has_next_candidate = candidates[index + 1..]
            .iter()
            .any(|next| !failed_credentials.contains(&next.credential_id));
        if !this.wait_for_candidate(candidate, has_next_candidate).await {
            route_trace.record_skipped(1);
            continue;
        }

        route_trace.record_attempt();
        let req =
            Request::from_parts(parts.clone(), Body::from(body_bytes.clone()));
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
                    route_trace: &mut route_trace,
                })
                .await?
            {
                return Ok(finish_success(
                    &this,
                    &mut route_trace,
                    candidate,
                    response,
                    status,
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
    );
    Err(ApiError::Internal(InternalError::ProviderNotFound))
}

fn finish_success(
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
    route_trace.emit(
        &this.router_id,
        this.strategy,
        &trace::RouteOutcome {
            label: "success",
            provider: Some(&candidate.capability.provider),
            credential: Some(&candidate.credential_id),
            status: Some(status.as_u16()),
        },
    );
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
    route_trace: &'a mut trace::RouteTrace,
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
        failed_credentials,
        route_trace,
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
                    failed_credentials,
                )
                .await;
                route_trace.record_skipped(skipped);
            } else {
                let _ = this
                    .record_failure(
                        &candidate.credential_id,
                        &candidate.capability.provider,
                        response,
                        elapsed,
                    )
                    .await;
                failed_credentials.insert(candidate.credential_id.clone());
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
        elapsed,
    );
    if let Some(ds) = response.extensions().get::<trace::DeepSeekWebTrace>() {
        route_trace.record_deepseek_web(*ds);
    }
    Ok(Some(response))
}

async fn fail_over_candidate(
    this: &BudgetAwareRouter,
    candidate: &BudgetCandidate,
    remaining_candidates: &[BudgetCandidate],
    response: Response,
    elapsed: std::time::Duration,
    reason: impl AsRef<str>,
    failed_credentials: &mut HashSet<ProviderCredentialId>,
) -> usize {
    let status = response.status();
    let (response, class) = failure::record_classified_failure(
        this,
        &candidate.credential_id,
        &candidate.capability.provider,
        response,
        elapsed,
    )
    .await;
    let _ = response;
    let next_provider =
        next_distinct_provider(remaining_candidates, failed_credentials);
    failure::record_failover_metric(
        this,
        candidate,
        next_provider,
        reason.as_ref(),
        status,
        class,
    );
    failed_credentials.insert(candidate.credential_id.clone());
    skip_free_siblings_on_exhaustion(
        candidate,
        remaining_candidates,
        class,
        failed_credentials,
    )
}

fn skip_free_siblings_on_exhaustion(
    candidate: &BudgetCandidate,
    remaining_candidates: &[BudgetCandidate],
    class: FailoverClass,
    failed_credentials: &mut HashSet<ProviderCredentialId>,
) -> usize {
    if !matches!(
        class,
        FailoverClass::QuotaExhausted | FailoverClass::Overload
    ) {
        return 0;
    }
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
) -> Option<&'a InferenceProvider> {
    candidates
        .iter()
        .find(|next| !failed_credentials.contains(&next.credential_id))
        .map(|c| &c.capability.provider)
}
