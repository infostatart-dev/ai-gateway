use std::{
    future::Future,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};

use tower::{Layer, Service};

use crate::{
    app_state::AppState,
    config::router::RouterConfig,
    error::api::ApiError,
    types::{request::Request, response::Response, router::RouterId},
};

/// The Decision Engine layer is responsible for enforcing budget, traffic
/// shaping, and other policies before the request reaches the routing strategy.
#[derive(Clone)]
pub struct DecisionEngineLayer {
    app_state: AppState,
    router_id: RouterId,
    router_config: Arc<RouterConfig>,
}

impl DecisionEngineLayer {
    pub fn new(
        app_state: AppState,
        router_id: RouterId,
        router_config: Arc<RouterConfig>,
    ) -> Self {
        Self {
            app_state,
            router_id,
            router_config,
        }
    }
}

impl<S> Layer<S> for DecisionEngineLayer {
    type Service = DecisionEngineService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        DecisionEngineService {
            inner,
            app_state: self.app_state.clone(),
            router_id: self.router_id.clone(),
            router_config: self.router_config.clone(),
        }
    }
}

#[derive(Clone)]
pub struct DecisionEngineService<S> {
    inner: S,
    app_state: AppState,
    router_id: RouterId,
    router_config: Arc<RouterConfig>,
}

impl<S> Service<Request> for DecisionEngineService<S>
where
    S: Service<Request, Response = Response, Error = ApiError> + Clone + Send + 'static,
    S::Future: Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request) -> Self::Future {
        let mut inner = self.inner.clone();
        
        Box::pin(async move {
            let (parts, body) = req.into_parts();
            use http_body_util::BodyExt;
            let body_bytes = body.collect().await.map_err(|e| {
                ApiError::Internal(crate::error::internal::InternalError::CollectBodyError(e))
            })?.to_bytes();
            
            let mut modified_body = body_bytes.clone();
            
            if let Ok(mut json) = serde_json::from_slice::<serde_json::Value>(&body_bytes) {
                // TODO: Determine budget cap from tier/provider configuration.
                // For now, if max_tokens is requested and we want to enforce a cap, we modify it.
                // Let's assume a hardcoded cap for demonstration of the modification mechanics.
                let budget_cap = 4000;
                
                let mut mutated = false;
                if let Some(obj) = json.as_object_mut() {
                    if let Some(max_tokens) = obj.get("max_tokens").and_then(serde_json::Value::as_u64) {
                        if max_tokens > budget_cap {
                            obj.insert("max_tokens".to_string(), serde_json::json!(budget_cap));
                            mutated = true;
                        }
                    } else if let Some(max_tokens) = obj.get("max_completion_tokens").and_then(serde_json::Value::as_u64) {
                        if max_tokens > budget_cap {
                            obj.insert("max_completion_tokens".to_string(), serde_json::json!(budget_cap));
                            mutated = true;
                        }
                    } else {
                        // If no max_tokens was specified, we inject a cap to prevent infinite streaming.
                        obj.insert("max_tokens".to_string(), serde_json::json!(budget_cap));
                        mutated = true;
                    }
                }
                
                if mutated {
                    #[allow(clippy::collapsible_if)]
                    if let Ok(new_bytes) = serde_json::to_vec(&json) {
                        modified_body = bytes::Bytes::from(new_bytes);
                    }
                }
            }

            // Reconstruct the request with the potentially modified body
            let req = Request::from_parts(parts, axum_core::body::Body::from(modified_body));

            // TODO: Traffic shaping (acquire slots)
            // Example usage:
            // let shaper = self.app_state.traffic_shaper();
            // let _permit = shaper.acquire(is_free, timeout).await?;

            // TODO: Budget check/reservation
            
            // TODO: Hedging / Emulated delay orchestration
            // e.g. let orchestrator = HedgingOrchestrator::new(Duration::from_millis(500));

            inner.call(req).await
        })
    }
}
