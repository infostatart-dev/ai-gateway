use std::{
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};

use axum_core::body::Body;
use bytes::Bytes;
use http_body::{Body as HttpBody, Frame};
use pin_project_lite::pin_project;

use crate::middleware::decision::{
    budget::StateStore, shaping::CombinedPermit,
};

pin_project! {
    pub struct DecisionBody {
        #[pin]
        inner: Body,
        state_store: Arc<dyn StateStore>,
        key: String,
        reservation_id: String,
        commit_amount: i64,
        commit_on_end: bool,
        permit: Option<CombinedPermit>,
        resolved: bool,
    }
    impl PinnedDrop for DecisionBody {
        fn drop(this: Pin<&mut Self>) {
            let this = this.project();
            if !*this.resolved {
                *this.resolved = true;
                let _permit = this.permit.take();
                refund(
                    this.state_store.clone(),
                    this.key.clone(),
                    this.reservation_id.clone(),
                );
            }
        }
    }
}

impl DecisionBody {
    #[must_use]
    pub fn new(
        inner: Body,
        state_store: Arc<dyn StateStore>,
        key: String,
        reservation_id: String,
        commit_amount: i64,
        commit_on_end: bool,
        permit: CombinedPermit,
    ) -> Self {
        Self {
            inner,
            state_store,
            key,
            reservation_id,
            commit_amount,
            commit_on_end,
            permit: Some(permit),
            resolved: false,
        }
    }
}

impl HttpBody for DecisionBody {
    type Data = Bytes;
    type Error = axum_core::Error;

    fn poll_frame(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        let mut this = self.project();
        match futures::ready!(this.inner.as_mut().poll_frame(cx)) {
            Some(Ok(frame)) => Poll::Ready(Some(Ok(frame))),
            Some(Err(err)) => {
                if !*this.resolved {
                    *this.resolved = true;
                    let _permit = this.permit.take();
                    refund(
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
                        commit(
                            this.state_store.clone(),
                            this.key.clone(),
                            this.reservation_id.clone(),
                            *this.commit_amount,
                        );
                    } else {
                        refund(
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
}

fn commit(
    store: Arc<dyn StateStore>,
    key: String,
    reservation_id: String,
    amount: i64,
) {
    tokio::spawn(async move {
        if let Err(error) = store
            .commit_reservation(&key, &reservation_id, amount)
            .await
        {
            tracing::warn!(%error, "failed to commit decision reservation");
        }
    });
}

fn refund(store: Arc<dyn StateStore>, key: String, reservation_id: String) {
    tokio::spawn(async move {
        if let Err(error) =
            store.refund_reservation(&key, &reservation_id).await
        {
            tracing::warn!(%error, "failed to refund decision reservation");
        }
    });
}
