use std::collections::HashSet;

use axum_core::body::Body;
use http::request::Parts;
use http_body_util::BodyExt;

use super::{
    call, structured_output,
    types::{BudgetAwareRouter, BudgetCandidate},
};
use crate::{
    config::credentials::ProviderCredentialId,
    error::{api::ApiError, internal::InternalError},
    router::{
        capability::RequestRequirements,
        provider_attempt::is_failoverable_status,
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

    for (index, candidate) in candidates.iter().enumerate() {
        if failed_credentials.contains(&candidate.credential_id) {
            continue;
        }

        let has_next_candidate = candidates[index + 1..]
            .iter()
            .any(|next| !failed_credentials.contains(&next.credential_id));
        if !this.wait_for_candidate(candidate, has_next_candidate).await {
            continue;
        }

        let req =
            Request::from_parts(parts.clone(), Body::from(body_bytes.clone()));
        let start = std::time::Instant::now();
        let mut response = call::call_candidate(candidate, req).await?;
        let elapsed = start.elapsed();
        let status = response.status();

        if has_next_candidate && is_failoverable_status(status) {
            fail_over_candidate(
                &this,
                candidate,
                &candidates[index + 1..],
                response,
                elapsed,
                crate::metrics::router::status_class(status),
                &mut failed_credentials,
            )
            .await;
            continue;
        }

        if status.is_success() {
            if let Some(mut response) =
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
                })
                .await?
            {
                attach_routed_identity(
                    &mut response,
                    &candidate.credential_id,
                    &candidate.capability.model,
                );
                return Ok(response);
            }
            continue;
        }

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
        return Ok(response);
    }

    Err(ApiError::Internal(InternalError::ProviderNotFound))
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
                fail_over_candidate(
                    this,
                    candidate,
                    &candidates[index + 1..],
                    response,
                    elapsed,
                    "structured_output",
                    failed_credentials,
                )
                .await;
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
                    "provider returned invalid structured JSON, failing over"
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
) {
    let next_provider =
        next_distinct_provider(remaining_candidates, failed_credentials);
    this.app_state.runtime_metrics().record_failover(
        &this.router_id,
        this.endpoint_type.as_ref(),
        this.strategy,
        &candidate.capability.provider,
        next_provider,
        reason.as_ref(),
    );
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
        reason = reason.as_ref(),
        "budget-aware router failed over to next candidate"
    );
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
