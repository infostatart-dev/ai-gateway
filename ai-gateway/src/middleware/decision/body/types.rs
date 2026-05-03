use std::sync::Arc;

use axum_core::body::Body;
use pin_project_lite::pin_project;

use super::settle;
use crate::middleware::decision::{
    budget::StateStore, shaping::CombinedPermit,
};

pin_project! {
    pub struct DecisionBody {
        #[pin]
        pub(super) inner: Body,
        pub(super) state_store: Arc<dyn StateStore>,
        pub(super) key: String,
        pub(super) reservation_id: String,
        pub(super) commit_amount: i64,
        pub(super) commit_on_end: bool,
        pub(super) permit: Option<CombinedPermit>,
        pub(super) resolved: bool,
    }
    impl PinnedDrop for DecisionBody {
        fn drop(this: Pin<&mut Self>) {
            let this = this.project();
            if !*this.resolved {
                *this.resolved = true;
                let _permit = this.permit.take();
                settle::refund(
                    this.state_store.clone(),
                    this.key.clone(),
                    this.reservation_id.clone(),
                );
            }
        }
    }
}
