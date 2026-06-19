mod parse;

use std::{
    sync::Arc,
    task::{Context, Poll},
};

pub use parse::{
    DEFAULT_AGENT_NAME, parse_agent_name, parse_work_unit_id, resolve_work_unit,
};

use crate::{
    config::router::RouterConfig,
    types::{
        extensions::CallerRequestContext, request::Request, response::Response,
    },
};

#[derive(Debug, Clone)]
pub struct Service<S> {
    inner: S,
    router_config: Option<Arc<RouterConfig>>,
}

impl<S> Service<S> {
    pub fn new(inner: S, router_config: Option<Arc<RouterConfig>>) -> Self {
        Self {
            inner,
            router_config,
        }
    }
}

impl<S> tower::Service<Request> for Service<S>
where
    S: tower::Service<Request, Response = Response> + Send + 'static,
    S::Future: Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = S::Future;

    #[inline]
    fn poll_ready(
        &mut self,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx).map_err(Into::into)
    }

    #[tracing::instrument(level = "debug", name = "caller_context", skip_all)]
    fn call(&mut self, mut req: Request) -> Self::Future {
        if self.router_config.is_some() {
            let headers = req.headers();
            let (work_unit_id, work_unit_source) = resolve_work_unit(headers);
            let caller = CallerRequestContext {
                agent_name: parse_agent_name(headers),
                work_unit_id: Some(work_unit_id),
                work_unit_source,
            };
            req.extensions_mut().insert(caller);
        }
        self.inner.call(req)
    }
}

#[derive(Debug, Clone)]
pub struct Layer {
    router_config: Option<Arc<RouterConfig>>,
}

impl Layer {
    #[must_use]
    pub fn for_router(router_config: Arc<RouterConfig>) -> Self {
        Self {
            router_config: Some(router_config),
        }
    }
}

impl<S> tower::Layer<S> for Layer {
    type Service = Service<S>;

    fn layer(&self, inner: S) -> Self::Service {
        Service::new(inner, self.router_config.clone())
    }
}
