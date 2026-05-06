use crate::{
    app_state::AppState,
    error::{api::ApiError, auth::AuthError, internal::InternalError},
    middleware::decision::{policy::KeyPolicy, shaping::CombinedPermit},
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
    let shaper_config = &app_state.config().decision.shaper;
    let outcome = app_state
        .0
        .traffic_shaper
        .acquire_with_cascade(
            policy.tier,
            shaper_config.cascade,
            shaper_config.acquire_timeout,
        )
        .await
        .map_err(|error| {
            tracing::warn!(%error, "traffic shaper rejected request");
            ApiError::Internal(InternalError::DecisionEngineError(error))
        })?;

    if outcome.tier != policy.tier {
        tracing::info!(
            requested = ?policy.tier,
            acquired = ?outcome.tier,
            "traffic shaper cascade fallback",
        );
    }
    Ok(outcome.permit)
}
