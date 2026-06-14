use std::collections::HashSet;

use axum_core::body::Body;
use http::request::Parts;
use http_body_util::BodyExt;

use super::{
    call,
    structured_output,
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

        let has_next_candidate = candidates[index + 1..].iter().any(|next| {
            !failed_credentials.contains(&next.credential_id)
        });
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
            let next_provider = next_distinct_provider(
                &candidates[index + 1..],
                &failed_credentials,
            );
            this.app_state.runtime_metrics().record_failover(
                &this.router_id,
                this.endpoint_type.as_ref(),
                this.strategy,
                &candidate.capability.provider,
                next_provider,
                crate::metrics::router::status_class(status),
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
                status = %status,
                "budget-aware router failed over to next candidate"
            );
            continue;
        }

        if status.is_success() {
            let has_next = index + 1 < candidates.len();
            if requirements.json_schema_required
                && candidate.capability.supports_json_schema
                && !structured_output::request_is_stream(&body_bytes)
            {
                let (response_parts, response_body) = response.into_parts();
                let response_bytes = response_body
                    .collect()
                    .await
                    .map_err(InternalError::CollectBodyError)?
                    .to_bytes();
                response = Response::from_parts(
                    response_parts,
                    Body::from(response_bytes.clone()),
                );

                if !structured_output::structured_output_valid(
                    &requirements,
                    &candidate.capability,
                    &body_bytes,
                    &response_bytes,
                ) {
                    let next_provider = next_distinct_provider(
                        &candidates[index + 1..],
                        &failed_credentials,
                    );
                    if has_next {
                        this.app_state.runtime_metrics().record_failover(
                            &this.router_id,
                            this.endpoint_type.as_ref(),
                            this.strategy,
                            &candidate.capability.provider,
                            next_provider,
                            "structured_output",
                        );
                    }
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
                    continue;
                }
            }

            this.record_success(
                &candidate.credential_id,
                &candidate.capability.provider,
                elapsed,
            );
        } else if is_failoverable_status(status) {
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

fn next_distinct_provider<'a>(
    candidates: &'a [BudgetCandidate],
    failed_credentials: &HashSet<ProviderCredentialId>,
) -> Option<&'a InferenceProvider> {
    candidates
        .iter()
        .find(|next| !failed_credentials.contains(&next.credential_id))
        .map(|c| &c.capability.provider)
}
