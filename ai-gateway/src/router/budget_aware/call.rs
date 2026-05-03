use std::convert::Infallible;

use tower::ServiceExt;

use super::types::BudgetCandidate;
use crate::{
    error::api::ApiError,
    types::{request::Request, response::Response},
};

pub(super) async fn call_candidate(
    candidate: &BudgetCandidate,
    req: Request,
) -> Result<Response, ApiError> {
    candidate
        .service
        .clone()
        .oneshot(req)
        .await
        .map_err(infallible_to_api_error)
}

pub(super) fn infallible_to_api_error(error: Infallible) -> ApiError {
    match error {}
}
