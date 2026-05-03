use std::sync::Arc;

use axum_core::body::Body;

use super::types::DecisionBody;
use crate::middleware::decision::{
    budget::StateStore, shaping::CombinedPermit,
};

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
