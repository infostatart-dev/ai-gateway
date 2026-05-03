use std::{
    future::Future,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};

use http_body_util::BodyExt;
use tower::{Layer, Service};

use crate::{
    app_state::AppState,
    config::router::RouterConfig,
    endpoints::{ApiEndpoint, EndpointType},
    error::{
        api::ApiError, auth::AuthError, internal::InternalError,
        invalid_req::InvalidRequestError,
    },
    middleware::decision::{
        policy::{KeyPolicy, Tier},
        shaping::CombinedPermit,
    },
    types::{
        extensions::AuthContext, request::Request, response::Response,
        router::RouterId,
    },
};

#[derive(Clone)]
pub struct DecisionEngineLayer {
    app_state: AppState,
}

impl DecisionEngineLayer {
    #[must_use]
    pub fn new(
        app_state: AppState,
        _router_id: RouterId,
        _router_config: Arc<RouterConfig>,
    ) -> Self {
        Self { app_state }
    }
}

impl<S> Layer<S> for DecisionEngineLayer {
    type Service = DecisionEngineService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        DecisionEngineService {
            inner,
            app_state: self.app_state.clone(),
        }
    }
}

#[derive(Clone)]
pub struct DecisionEngineService<S> {
    inner: S,
    app_state: AppState,
}

impl<S> Service<Request> for DecisionEngineService<S>
where
    S: Service<Request, Response = Response, Error = ApiError>
        + Clone
        + Send
        + 'static,
    S::Future: Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = Pin<
        Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>,
    >;

    fn poll_ready(
        &mut self,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request) -> Self::Future {
        let mut inner = self.inner.clone();
        std::mem::swap(&mut self.inner, &mut inner);
        let app_state = self.app_state.clone();

        Box::pin(async move {
            if !app_state.config().decision.enabled {
                return inner.call(req).await;
            }
            handle_decision_request(&mut inner, app_state, req).await
        })
    }
}

async fn handle_decision_request<S>(
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
        resolve_policy(&app_state, existing_policy, auth.as_ref()).await?;
    let permit = acquire_traffic_slot(&app_state, &policy).await?;
    let prepared = prepare_request(req, &policy).await?;
    let state_store = app_state.0.state_store.clone();
    let budget_key = policy.budget_namespace.clone();
    let reservation_id = state_store
        .reserve(&budget_key, prepared.reserved_output_tokens)
        .await
        .map_err(|error| {
            tracing::warn!(%error, "budget reservation failed");
            ApiError::Internal(InternalError::DecisionEngineError(error))
        })?;

    match inner.call(prepared.request).await {
        Ok(response) => Ok(wrap_response_body(
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

async fn resolve_policy(
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

async fn acquire_traffic_slot(
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

struct PreparedDecisionRequest {
    request: Request,
    reserved_output_tokens: i64,
}

async fn prepare_request(
    req: Request,
    policy: &KeyPolicy,
) -> Result<PreparedDecisionRequest, ApiError> {
    let applies_token_policy = req
        .extensions()
        .get::<ApiEndpoint>()
        .is_none_or(|endpoint| endpoint.endpoint_type() == EndpointType::Chat);
    let (mut parts, body) = req.into_parts();
    let body_bytes = body
        .collect()
        .await
        .map_err(|error| {
            ApiError::Internal(InternalError::CollectBodyError(error))
        })?
        .to_bytes();
    let mut modified_body = body_bytes.clone();
    let mut reserved_output_tokens = u64::from(policy.max_output_tokens);

    if applies_token_policy {
        let policy_result = apply_token_policy(&body_bytes, policy)?;
        reserved_output_tokens = policy_result.reserved_output_tokens;
        if let Some(body) = policy_result.modified_body {
            parts.headers.remove(http::header::CONTENT_LENGTH);
            modified_body = body;
        }
    }

    Ok(PreparedDecisionRequest {
        request: Request::from_parts(
            parts,
            axum_core::body::Body::from(modified_body),
        ),
        reserved_output_tokens: i64::try_from(reserved_output_tokens).map_err(
            |_| {
                ApiError::InvalidRequest(InvalidRequestError::BudgetExceeded(
                    "max_tokens exceeds supported budget range".to_string(),
                ))
            },
        )?,
    })
}

struct TokenPolicyResult {
    reserved_output_tokens: u64,
    modified_body: Option<bytes::Bytes>,
}

fn apply_token_policy(
    body: &bytes::Bytes,
    policy: &KeyPolicy,
) -> Result<TokenPolicyResult, ApiError> {
    let output_cap = u64::from(policy.max_output_tokens);
    let Ok(mut json) = serde_json::from_slice::<serde_json::Value>(body) else {
        return Ok(TokenPolicyResult {
            reserved_output_tokens: output_cap,
            modified_body: None,
        });
    };
    let Some(obj) = json.as_object_mut() else {
        return Ok(TokenPolicyResult {
            reserved_output_tokens: output_cap,
            modified_body: None,
        });
    };

    if let Some(requested_output_tokens) = requested_output_tokens(obj) {
        if requested_output_tokens > output_cap {
            return Err(ApiError::InvalidRequest(
                InvalidRequestError::BudgetExceeded(format!(
                    "max_tokens exceeds budget cap of {output_cap}"
                )),
            ));
        }
        return Ok(TokenPolicyResult {
            reserved_output_tokens: requested_output_tokens,
            modified_body: None,
        });
    }

    obj.insert("max_tokens".to_string(), serde_json::json!(output_cap));
    let modified_body = serde_json::to_vec(&json)
        .map(bytes::Bytes::from)
        .map_err(InvalidRequestError::InvalidRequestBody)?;
    Ok(TokenPolicyResult {
        reserved_output_tokens: output_cap,
        modified_body: Some(modified_body),
    })
}

fn requested_output_tokens(
    obj: &serde_json::Map<String, serde_json::Value>,
) -> Option<u64> {
    obj.get("max_tokens")
        .or_else(|| obj.get("max_completion_tokens"))
        .and_then(serde_json::Value::as_u64)
}

fn wrap_response_body(
    response: Response,
    state_store: Arc<dyn crate::middleware::decision::budget::StateStore>,
    budget_key: String,
    reservation_id: String,
    reserved_output_tokens: i64,
    permit: CombinedPermit,
) -> Response {
    let (parts, body) = response.into_parts();
    let commit_on_end = parts.status.is_success();
    let body = crate::middleware::decision::body::DecisionBody::new(
        body,
        state_store,
        budget_key,
        reservation_id,
        reserved_output_tokens,
        commit_on_end,
        permit,
    );
    Response::from_parts(parts, axum_core::body::Body::new(body))
}

#[cfg(test)]
mod tests {
    use bytes::Bytes;
    use serde_json::json;

    use super::*;

    fn policy(max_output_tokens: u32) -> KeyPolicy {
        KeyPolicy {
            tier: Tier::Free,
            budget_namespace: "test".to_string(),
            max_output_tokens,
            allow_hedging: false,
            allow_delay: false,
        }
    }

    #[test]
    fn token_policy_injects_missing_max_tokens() {
        let body = Bytes::from(json!({ "model": "gpt-4o-mini" }).to_string());

        let result = apply_token_policy(&body, &policy(128)).unwrap();

        assert_eq!(result.reserved_output_tokens, 128);
        let modified = result.modified_body.unwrap();
        let value: serde_json::Value =
            serde_json::from_slice(&modified).unwrap();
        assert_eq!(value["max_tokens"], 128);
    }

    #[test]
    fn token_policy_rejects_excessive_max_tokens() {
        let body = Bytes::from(
            json!({ "model": "gpt-4o-mini", "max_tokens": 129 }).to_string(),
        );

        let result = apply_token_policy(&body, &policy(128));

        assert!(matches!(
            result,
            Err(ApiError::InvalidRequest(
                InvalidRequestError::BudgetExceeded(_)
            ))
        ));
    }
}
