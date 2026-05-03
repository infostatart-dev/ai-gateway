use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};

use tower::Service;

use super::{handle, layer::DecisionEngineService};
use crate::{
    error::api::ApiError,
    types::{request::Request, response::Response},
};

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
            handle::handle_decision_request(&mut inner, app_state, req).await
        })
    }
}
