mod types;
mod version;
mod merge;
pub mod request;
pub mod variables;

pub use request::build_prompt_request;

use crate::app_state::AppState;
use crate::error::api::ApiError;
use crate::error::init::InitError;
use crate::types::request::Request;
use crate::types::response::Response;
use futures::future::BoxFuture;
use std::task::{Context, Poll};
use tower::{Layer, Service};

#[derive(Clone, Default)]
pub struct PromptLayer;

impl PromptLayer {
    pub fn new(_app_state: &AppState) -> Result<Self, InitError> {
        Ok(Self)
    }
}

impl<S> Layer<S> for PromptLayer {
    type Service = PromptService<S>;
    fn layer(&self, inner: S) -> Self::Service { PromptService { inner } }
}

#[derive(Clone)]
pub struct PromptService<S> {
    inner: S,
}

impl<S> Service<Request> for PromptService<S>
where
    S: Service<Request, Response = Response, Error = ApiError> + Clone + Send + 'static,
    S::Future: Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> { self.inner.poll_ready(cx) }

    fn call(&mut self, req: Request) -> Self::Future {
        let mut inner = self.inner.clone();
        let app_state = req.extensions().get::<AppState>().cloned().expect("AppState not found in request extensions");
        Box::pin(async move {
            let req = request::build_prompt_request(app_state, req).await?;
            inner.call(req).await
        })
    }
}
