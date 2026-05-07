use std::task::{Context, Poll};

use tower::{Layer, Service};

use crate::{
    app_state::AppState,
    endpoints::EndpointType,
    metrics::router::attrs,
    types::{extensions::RouterRuntimeLabels, request, router::RouterId},
};

#[derive(Clone)]
pub struct RouterMetricsLayer {
    app_state: AppState,
    router_id: RouterId,
    endpoint_type: EndpointType,
    strategy: &'static str,
}

impl RouterMetricsLayer {
    #[must_use]
    pub fn new(
        app_state: AppState,
        router_id: RouterId,
        endpoint_type: EndpointType,
        strategy: &'static str,
    ) -> Self {
        Self {
            app_state,
            router_id,
            endpoint_type,
            strategy,
        }
    }
}

impl<S> Layer<S> for RouterMetricsLayer {
    type Service = RouterMetricsService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        RouterMetricsService {
            inner,
            app_state: self.app_state.clone(),
            router_id: self.router_id.clone(),
            endpoint_type: self.endpoint_type,
            strategy: self.strategy,
        }
    }
}

#[derive(Clone)]
pub struct RouterMetricsService<S> {
    inner: S,
    app_state: AppState,
    router_id: RouterId,
    endpoint_type: EndpointType,
    strategy: &'static str,
}

impl<S> Service<request::Request> for RouterMetricsService<S>
where
    S: Service<request::Request, Response = crate::types::response::Response>,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = S::Future;

    fn poll_ready(
        &mut self,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, mut req: request::Request) -> Self::Future {
        let rtl = RouterRuntimeLabels {
            router_id: self.router_id.clone(),
            endpoint_type: self.endpoint_type.as_ref().to_string(),
            strategy: self.strategy,
        };
        let attrs = attrs::base_router_kv(&rtl);
        self.app_state
            .0
            .metrics
            .runtime
            .router_requests_total
            .add(1, &attrs);

        req.extensions_mut().insert(rtl);

        self.inner.call(req)
    }
}
