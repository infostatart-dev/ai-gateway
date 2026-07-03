//! Upstream pacing: catalog-driven concurrent / RPM / min-interval gates per
//! provider account scope.

mod daily;
pub(crate) mod gate;
pub(crate) mod limits;
mod registry;
mod scope;
mod tpm;
mod window;

use gate::PacingPermit;
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
    tier: Option<&str>,
    model: Option<&str>,
    estimated_tokens: u32,
) -> Result<Option<PacingPermit>, ApiError> {
    let Some(gate) = app_state.upstream_pacing().gate_for(
        provider,
        credential_id,
        tier,
        model,
    ) else {
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
        Err(retry_after) => {
            tracing::event!(
                tracing::Level::INFO,
                blocked_reason = "pacing",
                wait_ms = u64::try_from(retry_after.as_millis())
                    .unwrap_or(u64::MAX),
                retry_after_ms = u64::try_from(retry_after.as_millis())
                    .unwrap_or(u64::MAX),
                provider = %provider,
                credential = credential_id.map_or(
                    "none",
                    ProviderCredentialId::as_str,
                ),
                model = model.unwrap_or("none"),
                tier = tier.unwrap_or("none"),
                estimated_tokens,
                "gateway.pacing.wait"
            );
            Err(ApiError::InvalidRequest(
                InvalidRequestError::TooManyRequests(
                    crate::error::invalid_req::TooManyRequestsError {
                        ratelimit_limit,
                        ratelimit_remaining: 0,
                        retry_after: retry_after.as_secs().max(1),
                    },
                ),
            ))
        }
    }
}
