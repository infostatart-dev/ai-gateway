use axum_core::response::Response;
use bytes::Bytes;
// use http::StatusCode;
use http_body::Body;
use std::{
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
};

/// An SSE body that periodically emits keep-alive comments before streaming the actual response.
pub struct DelayedSseBody<B> {
    inner: Option<B>,
    delay_until: tokio::time::Instant,
    interval: tokio::time::Interval,
}

impl<B> DelayedSseBody<B> {
    pub fn new(inner: B, delay: Duration) -> Self {
        let mut interval = tokio::time::interval(Duration::from_secs(1));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        Self {
            inner: Some(inner),
            delay_until: tokio::time::Instant::now() + delay,
            interval,
        }
    }
}

impl<B> Body for DelayedSseBody<B>
where
    B: Body<Data = Bytes> + Unpin,
{
    type Data = Bytes;
    type Error = B::Error;

    fn poll_frame(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<http_body::Frame<Self::Data>, Self::Error>>> {
        if tokio::time::Instant::now() < self.delay_until {
            // Still in the delay phase, poll the interval for keep-alives
            match self.interval.poll_tick(cx) {
                Poll::Ready(_) => {
                    let keep_alive = Bytes::from(": keep-alive\n\n");
                    return Poll::Ready(Some(Ok(http_body::Frame::data(keep_alive))));
                }
                Poll::Pending => return Poll::Pending,
            }
        }

        // Delay phase over, stream the actual inner body
        if let Some(inner) = self.inner.as_mut() {
            Pin::new(inner).poll_frame(cx)
        } else {
            Poll::Ready(None)
        }
    }
}

/// Helper function to wrap a response in a bounded emulated delay SSE stream.
pub fn apply_bounded_delay<B>(response: Response<B>, delay: Duration) -> Response<DelayedSseBody<B>>
where
    B: Body<Data = Bytes> + Unpin,
{
    let (parts, body) = response.into_parts();
    Response::from_parts(parts, DelayedSseBody::new(body, delay))
}
