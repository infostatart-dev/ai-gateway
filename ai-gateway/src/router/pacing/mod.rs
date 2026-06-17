//! Upstream pacing: catalog-driven concurrent / RPM / min-interval gates per
//! provider account scope.

mod daily;
mod gate;
mod limits;
mod registry;
mod scope;
mod tpm;
mod window;

pub use gate::{PacingGate, PacingPermit};
pub use limits::PacingLimits;
pub use registry::PacingRegistry;

use crate::{
    app_state::AppState,
    config::credentials::ProviderCredentialId,
    error::{api::ApiError, invalid_req::InvalidRequestError},
    types::provider::InferenceProvider,
};

/// Acquire pacing permit before upstream dispatch (shared by proxy and embedded
/// executors).
pub async fn acquire_upstream_pacing(
    app_state: &AppState,
    provider: &InferenceProvider,
    credential_id: Option<&ProviderCredentialId>,
    estimated_tokens: u32,
) -> Result<Option<PacingPermit>, ApiError> {
    let Some(gate) = app_state
        .upstream_pacing()
        .gate_for(provider, credential_id)
    else {
        return Ok(None);
    };
    let limits = gate.limits();
    let ratelimit_limit = u64::from(if limits.has_rpm_limit() {
        limits.rpm
    } else {
        limits.rpd.or(limits.tpm).unwrap_or(1)
    });
    match gate.acquire(estimated_tokens).await {
        Ok(permit) => Ok(Some(permit)),
        Err(retry_after) => Err(ApiError::InvalidRequest(
            InvalidRequestError::TooManyRequests(
                crate::error::invalid_req::TooManyRequestsError {
                    ratelimit_limit,
                    ratelimit_remaining: 0,
                    retry_after: retry_after.as_secs().max(1),
                },
            ),
        )),
    }
}
