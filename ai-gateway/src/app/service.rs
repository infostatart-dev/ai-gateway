use std::{task::{Context, Poll}, convert::Infallible};
use futures::future::BoxFuture;
use crate::app::App;

impl tower::Service<crate::types::request::Request> for App {
    type Response = super::AppResponse;
    type Error = Infallible;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    #[inline]
    #[tracing::instrument(skip_all)]
    fn poll_ready(&mut self, ctx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.service_stack.poll_ready(ctx)
    }

    #[inline]
    fn call(&mut self, req: crate::types::request::Request) -> Self::Future {
        tracing::trace!(uri = %req.uri(), method = %req.method(), version = ?req.version(), "app received request");
        self.service_stack.call(req)
    }
}
