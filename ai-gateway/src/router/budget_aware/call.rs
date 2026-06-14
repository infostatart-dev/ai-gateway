use std::convert::Infallible;

use tower::ServiceExt;

use super::types::BudgetCandidate;
use crate::{
    error::api::ApiError,
    types::{request::Request, response::Response},
};

#[cfg(all(test, feature = "testing"))]
mod test_hooks {
    use std::collections::VecDeque;
    use std::sync::{Mutex, OnceLock};

    use super::{ApiError, Response};

    static MOCK_CALLS: OnceLock<Mutex<VecDeque<Result<Response, ApiError>>>> =
        OnceLock::new();

    fn queue() -> &'static Mutex<VecDeque<Result<Response, ApiError>>> {
        MOCK_CALLS.get_or_init(|| Mutex::new(VecDeque::new()))
    }

    pub fn push(response: Result<Response, ApiError>) {
        queue().lock().expect("mock call queue").push_back(response);
    }

    pub fn pop() -> Option<Result<Response, ApiError>> {
        queue().lock().expect("mock call queue").pop_front()
    }

    pub fn clear() {
        if let Some(queue) = MOCK_CALLS.get() {
            queue.lock().expect("mock call queue").clear();
        }
    }
}

#[cfg(all(test, feature = "testing"))]
pub use test_hooks::{clear as clear_test_call_responses, push as push_test_call_response};

pub(super) async fn call_candidate(
    candidate: &BudgetCandidate,
    req: Request,
) -> Result<Response, ApiError> {
    #[cfg(all(test, feature = "testing"))]
    if let Some(response) = test_hooks::pop() {
        return response;
    }

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
