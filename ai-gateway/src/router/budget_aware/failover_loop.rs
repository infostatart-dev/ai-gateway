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
    let mut failed_providers = HashSet::<InferenceProvider>::new();

    for (index, candidate) in candidates.iter().enumerate() {
        if failed_providers.contains(&candidate.capability.provider) {
            continue;
        }

        let has_next_provider = candidates[index + 1..].iter().any(|next| {
            next.capability.provider != candidate.capability.provider
                && !failed_providers.contains(&next.capability.provider)
        });
        if !this.wait_for_candidate(candidate, has_next_provider).await {
            continue;
        }

        let req =
            Request::from_parts(parts.clone(), Body::from(body_bytes.clone()));
        let start = std::time::Instant::now();
        let mut response = call::call_candidate(candidate, req).await?;
        let elapsed = start.elapsed();
        let status = response.status();

        if has_next_provider && is_failoverable_status(status) {
            let next_provider = candidates[index + 1..]
                .iter()
                .find(|next| {
                    next.capability.provider != candidate.capability.provider
                        && !failed_providers.contains(&next.capability.provider)
                })
                .map(|c| &c.capability.provider);
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
                    &candidate.capability.provider,
                    response,
                    elapsed,
                )
                .await;
            failed_providers.insert(candidate.capability.provider.clone());
            tracing::warn!(
                provider = %candidate.capability.provider,
                model = %candidate.capability.model,
                status = %status,
                "budget-aware router failed over to next candidate"
            );
            continue;
        }

        if status.is_success() {
            let has_next_candidate = index + 1 < candidates.len();
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
                    let next_provider = candidates[index + 1..]
                        .iter()
                        .find(|next| {
                            next.capability.provider
                                != candidate.capability.provider
                                && !failed_providers
                                    .contains(&next.capability.provider)
                        })
                        .map(|c| &c.capability.provider);
                    if has_next_candidate {
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
                            &candidate.capability.provider,
                            response,
                            elapsed,
                        )
                        .await;
                    failed_providers.insert(candidate.capability.provider.clone());
                    tracing::warn!(
                        provider = %candidate.capability.provider,
                        model = %candidate.capability.model,
                        "provider returned invalid structured JSON, failing over"
                    );
                    continue;
                }
            }

            this.record_success(&candidate.capability.provider, elapsed);
        } else if is_failoverable_status(status) {
            response = this
                .record_failure(
                    &candidate.capability.provider,
                    response,
                    elapsed,
                )
                .await;
        }
        attach_routed_identity(
            &mut response,
            &candidate.capability.provider,
            &candidate.capability.model,
        );
        return Ok(response);
    }

    Err(ApiError::Internal(InternalError::ProviderNotFound))
}
