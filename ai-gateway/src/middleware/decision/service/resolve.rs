use crate::{
    app_state::AppState,
    error::{api::ApiError, auth::AuthError, internal::InternalError},
    middleware::decision::{
        policy::{KeyPolicy, Tier},
        shaping::CombinedPermit,
    },
    types::extensions::AuthContext,
};

pub(super) async fn resolve_policy(
    app_state: &AppState,
    existing_policy: Option<KeyPolicy>,
    auth: Option<&AuthContext>,
) -> Result<KeyPolicy, ApiError> {
    if let Some(policy) = existing_policy {
        return Ok(policy);
    }
    if let Some(policy) = app_state.0.policy_store.get_policy(auth).await {
        return Ok(policy);
    }

    tracing::warn!("decision engine enabled without a resolved policy");
    Err(ApiError::Authentication(AuthError::InvalidCredentials))
}

pub(super) async fn acquire_traffic_slot(
    app_state: &AppState,
    policy: &KeyPolicy,
) -> Result<CombinedPermit, ApiError> {
    app_state
        .0
        .traffic_shaper
        .acquire(
            policy.tier == Tier::Free,
            app_state.config().decision.shaper.acquire_timeout,
        )
        .await
        .map_err(|error| {
            tracing::warn!(%error, "traffic shaper rejected request");
            ApiError::Internal(InternalError::DecisionEngineError(error))
        })
}
