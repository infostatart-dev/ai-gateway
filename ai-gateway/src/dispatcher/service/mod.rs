use std::task::{Context, Poll};
use tokio::sync::mpsc::Sender;
use tower::Service;
use futures::future::BoxFuture;
use crate::{
    app_state::AppState,
    dispatcher::client::Client,
    error::api::ApiError,
    types::{
        provider::InferenceProvider,
        rate_limit::RateLimitEvent,
        request::Request,
        body::Body,
    },
    middleware::{add_extension::AddExtensions, mapper::Service as MapperService},
    utils::handle_error::ErrorHandler,
};

pub mod factory;
pub mod dispatch;
pub mod dispatch_sync;
pub mod dispatch_stream;
pub mod retry;
pub mod logging;
pub mod utils;

pub type DispatcherFuture = BoxFuture<'static, Result<http::Response<Body>, ApiError>>;
pub type DispatcherService = AddExtensions<ErrorHandler<MapperService<Dispatcher>>>;
pub type DispatcherServiceWithoutMapper = AddExtensions<ErrorHandler<Dispatcher>>;

#[derive(Debug, Clone)]
pub struct Dispatcher {
    pub(crate) client: Client,
    pub(crate) app_state: AppState,
    pub(crate) provider: InferenceProvider,
    pub(crate) rate_limit_tx: Option<Sender<RateLimitEvent>>,
}

impl Service<Request> for Dispatcher {
    type Response = http::Response<Body>;
    type Error = ApiError;
    type Future = DispatcherFuture;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    #[tracing::instrument(name = "dispatcher", skip_all)]
    fn call(&mut self, req: Request) -> Self::Future {
        let this = self.clone();
        let this = std::mem::replace(self, this);
        Box::pin(async move { this.dispatch(req).await })
    }
}
