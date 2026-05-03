//! Response body wrapper: commit or refund budget on stream completion.

mod construct;
mod poll;
mod settle;
mod types;

use std::{
    pin::Pin,
    task::{Context, Poll},
};

use bytes::Bytes;
use http_body::{Body as HttpBody, Frame};
pub use types::DecisionBody;

impl HttpBody for DecisionBody {
    type Data = Bytes;
    type Error = axum_core::Error;

    fn poll_frame(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        poll::poll_decision_frame(self, cx)
    }
}
