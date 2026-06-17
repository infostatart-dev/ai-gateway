//! Upstream pacing: catalog-driven concurrent / RPM / min-interval gates per
//! provider account scope.

mod gate;
mod limits;
mod registry;
mod scope;
mod window;

pub use gate::PacingPermit;
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
) -> Result<Option<PacingPermit>, ApiError> {
    let Some(gate) = app_state
        .upstream_pacing()
        .gate_for(provider, credential_id)
    else {
        return Ok(None);
    };
    let rpm = gate.limits().rpm;
    match gate.acquire().await {
        Ok(permit) => Ok(Some(permit)),
        Err(retry_after) => Err(ApiError::InvalidRequest(
            InvalidRequestError::TooManyRequests(
                crate::error::invalid_req::TooManyRequestsError {
                    ratelimit_limit: u64::from(rpm),
                    ratelimit_remaining: 0,
                    retry_after: retry_after.as_secs().max(1),
                },
            ),
        )),
    }
}
