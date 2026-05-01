mod merge;
pub mod request;
mod types;
pub mod variables;
mod version;

use std::task::{Context, Poll};

use futures::future::BoxFuture;
pub use request::build_prompt_request;
use tower::{Layer, Service};

use crate::{
    app_state::AppState,
    error::{api::ApiError, init::InitError},
    types::{request::Request, response::Response},
};

#[derive(Clone)]
pub struct PromptLayer {
    app_state: AppState,
}

impl PromptLayer {
    pub fn new(app_state: &AppState) -> Result<Self, InitError> {
        Ok(Self {
            app_state: app_state.clone(),
        })
    }
}

impl<S> Layer<S> for PromptLayer {
    type Service = PromptService<S>;
    fn layer(&self, inner: S) -> Self::Service {
        PromptService {
            inner,
            app_state: self.app_state.clone(),
        }
    }
}

#[derive(Clone)]
pub struct PromptService<S> {
    inner: S,
    app_state: AppState,
}

impl<S> Service<Request> for PromptService<S>
where
    S: Service<Request, Response = Response, Error = ApiError>
        + Clone
        + Send
        + 'static,
    S::Future: Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(
        &mut self,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request) -> Self::Future {
        let mut this = self.clone();
        std::mem::swap(self, &mut this);
        let app_state = this.app_state.clone();
        Box::pin(async move {
            let req = request::build_prompt_request(app_state, req).await?;
            this.inner.call(req).await
        })
    }
}
