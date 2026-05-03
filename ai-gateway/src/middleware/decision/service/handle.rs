use tower::Service;

use super::{prepare, resolve, wrap};
use crate::{
    app_state::AppState,
    error::api::ApiError,
    middleware::decision::policy::KeyPolicy,
    types::{extensions::AuthContext, request::Request, response::Response},
};

pub(super) async fn handle_decision_request<S>(
    inner: &mut S,
    app_state: AppState,
    req: Request,
) -> Result<Response, ApiError>
where
    S: Service<Request, Response = Response, Error = ApiError>
        + Clone
        + Send
        + 'static,
    S::Future: Send + 'static,
{
    let auth = req.extensions().get::<AuthContext>().cloned();
    let existing_policy = req.extensions().get::<KeyPolicy>().cloned();
    let policy =
        resolve::resolve_policy(&app_state, existing_policy, auth.as_ref())
            .await?;
    let permit = resolve::acquire_traffic_slot(&app_state, &policy).await?;
    let prepared = prepare::prepare_request(req, &policy).await?;
    let state_store = app_state.0.state_store.clone();
    let budget_key = policy.budget_namespace.clone();
    let reservation_id = state_store
        .reserve(&budget_key, prepared.reserved_output_tokens)
        .await
        .map_err(|error| {
            tracing::warn!(%error, "budget reservation failed");
            ApiError::Internal(
                crate::error::internal::InternalError::DecisionEngineError(
                    error,
                ),
            )
        })?;

    match inner.call(prepared.request).await {
        Ok(response) => Ok(wrap::wrap_response_body(
            response,
            state_store,
            budget_key,
            reservation_id,
            prepared.reserved_output_tokens,
            permit,
        )),
        Err(error) => {
            let _ = state_store
                .refund_reservation(&budget_key, &reservation_id)
                .await;
            drop(permit);
            Err(error)
        }
    }
}
