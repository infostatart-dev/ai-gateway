use std::{
    pin::Pin,
    task::{Context, Poll},
};

use bytes::Bytes;
use futures::ready;
use http_body::{Body as HttpBody, Frame};

use super::{DecisionBody, settle};

pub(super) fn poll_decision_frame(
    body: Pin<&mut DecisionBody>,
    cx: &mut Context<'_>,
) -> Poll<Option<Result<Frame<Bytes>, axum_core::Error>>> {
    let mut this = body.project();
    match ready!(this.inner.as_mut().poll_frame(cx)) {
        Some(Ok(frame)) => Poll::Ready(Some(Ok(frame))),
        Some(Err(err)) => {
            if !*this.resolved {
                *this.resolved = true;
                let _permit = this.permit.take();
                settle::refund(
                    this.state_store.clone(),
                    this.key.clone(),
                    this.reservation_id.clone(),
                );
            }
            Poll::Ready(Some(Err(err)))
        }
        None => {
            if !*this.resolved {
                *this.resolved = true;
                let _permit = this.permit.take();
                if *this.commit_on_end {
                    settle::commit(
                        this.state_store.clone(),
                        this.key.clone(),
                        this.reservation_id.clone(),
                        *this.commit_amount,
                    );
                } else {
                    settle::refund(
                        this.state_store.clone(),
                        this.key.clone(),
                        this.reservation_id.clone(),
                    );
                }
            }
            Poll::Ready(None)
        }
    }
}
