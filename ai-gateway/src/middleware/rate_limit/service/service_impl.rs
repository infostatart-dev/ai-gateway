use std::task::{Context, Poll};

use http::Response;

use crate::middleware::rate_limit::service::{
    GovernorService, RedisRateLimitService, Service, future::ResponseFuture,
};

impl<S, Request, ResponseBody> tower::Service<Request> for Service<S>
where
    S: tower::Service<Request, Response = Response<ResponseBody>>,
    GovernorService<S>: tower::Service<
            Request,
            Response = Response<ResponseBody>,
            Error = S::Error,
        >,
    RedisRateLimitService<S>: tower::Service<
            Request,
            Response = Response<ResponseBody>,
            Error = S::Error,
        >,
{
    type Response = Response<ResponseBody>;
    type Error = S::Error;
    type Future = ResponseFuture<
        <GovernorService<S> as tower::Service<Request>>::Future,
        <RedisRateLimitService<S> as tower::Service<Request>>::Future,
        S::Future,
    >;

    fn poll_ready(
        &mut self,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), Self::Error>> {
        match self {
            Service::InMemory { service } => match service.poll_ready(cx) {
                Poll::Ready(Ok(())) => {
                    tracing::trace!("in memory rate limit ready");
                    Poll::Ready(Ok(()))
                }
                res => res,
            },
            Service::Redis { service } => service.poll_ready(cx),
            Service::Disabled { service } => service.poll_ready(cx),
        }
    }

    #[tracing::instrument(name = "opt_rate_limit", skip_all)]
    fn call(&mut self, req: Request) -> Self::Future {
        match self {
            Service::InMemory { service } => {
                tracing::trace!(kind = "in_memory", "rate limit middleware");
                ResponseFuture::InMemory {
                    future: service.call(req),
                }
            }
            Service::Redis { service } => {
                tracing::trace!(kind = "redis", "rate limit middleware");
                ResponseFuture::Redis {
                    future: service.call(req),
                }
            }
            Service::Disabled { service } => ResponseFuture::Disabled {
                future: service.call(req),
            },
        }
    }
}
