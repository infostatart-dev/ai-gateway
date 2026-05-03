use std::sync::Arc;

use crate::{
    middleware::decision::{
        body::DecisionBody, budget::StateStore, shaping::CombinedPermit,
    },
    types::response::Response,
};

pub(super) fn wrap_response_body(
    response: Response,
    state_store: Arc<dyn StateStore>,
    budget_key: String,
    reservation_id: String,
    reserved_output_tokens: i64,
    permit: CombinedPermit,
) -> Response {
    let (parts, body) = response.into_parts();
    let commit_on_end = parts.status.is_success();
    let body = DecisionBody::new(
        body,
        state_store,
        budget_key,
        reservation_id,
        reserved_output_tokens,
        commit_on_end,
        permit,
    );
    Response::from_parts(parts, axum_core::body::Body::new(body))
}
