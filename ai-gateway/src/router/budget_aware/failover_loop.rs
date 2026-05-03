use std::collections::HashSet;

use axum_core::body::Body;
use http::request::Parts;

use super::{
    call,
    types::{BudgetAwareRouter, BudgetCandidate},
};
use crate::{
    error::{api::ApiError, internal::InternalError},
    router::provider_attempt::is_failoverable_status,
    types::{
        provider::InferenceProvider, request::Request, response::Response,
    },
};

pub(super) async fn run_failover_candidates(
    this: BudgetAwareRouter,
    parts: Parts,
    body_bytes: bytes::Bytes,
    candidates: Vec<BudgetCandidate>,
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
        let response = call::call_candidate(candidate, req).await?;
        let elapsed = start.elapsed();
        let status = response.status();

        if has_next_provider && is_failoverable_status(status) {
            this.record_failure(
                &candidate.capability.provider,
                &response,
                elapsed,
            );
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
            this.record_success(&candidate.capability.provider, elapsed);
        } else if is_failoverable_status(status) {
            this.record_failure(
                &candidate.capability.provider,
                &response,
                elapsed,
            );
        }
        return Ok(response);
    }

    Err(ApiError::Internal(InternalError::ProviderNotFound))
}
