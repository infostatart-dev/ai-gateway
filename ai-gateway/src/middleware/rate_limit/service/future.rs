use std::{future::Future, pin::Pin, task::{Context, Poll}};
use http::Response;
use super::utils::increment_retry_after_header;

pin_project_lite::pin_project! {
    #[derive(Debug)]
    #[project = EnumProj]
    pub enum ResponseFuture<InMemoryFuture, RedisFuture, DisabledFuture> {
        InMemory { #[pin] future: InMemoryFuture },
        Redis { #[pin] future: RedisFuture },
        Disabled { #[pin] future: DisabledFuture },
    }
}

impl<InMemoryFuture, RedisFuture, DisabledFuture, ResponseBody, Error> Future
    for ResponseFuture<InMemoryFuture, RedisFuture, DisabledFuture>
where
    InMemoryFuture: Future<Output = Result<Response<ResponseBody>, Error>>,
    RedisFuture: Future<Output = Result<Response<ResponseBody>, Error>>,
    DisabledFuture: Future<Output = Result<Response<ResponseBody>, Error>>,
{
    type Output = Result<Response<ResponseBody>, Error>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match self.project() {
            EnumProj::InMemory { future } => {
                let result = std::task::ready!(future.poll(cx));
                if let Ok(mut res) = result { increment_retry_after_header(&mut res); Poll::Ready(Ok(res)) }
                else { Poll::Ready(result) }
            }
            EnumProj::Redis { future } => future.poll(cx),
            EnumProj::Disabled { future } => future.poll(cx),
        }
    }
}
