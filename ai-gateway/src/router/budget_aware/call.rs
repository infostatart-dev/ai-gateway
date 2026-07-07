use std::convert::Infallible;

use tower::ServiceExt;

use super::types::BudgetCandidate;
use crate::{
    error::api::ApiError,
    types::{request::Request, response::Response},
};

#[cfg(feature = "testing")]
mod test_hooks {
    use std::{
        collections::{HashMap, VecDeque},
        sync::{Mutex, OnceLock},
    };

    use super::{ApiError, Response};

    pub enum MockCallResponse {
        Immediate(Result<Response, ApiError>),
        #[cfg(test)]
        Delayed(std::time::Duration, Result<Response, ApiError>),
        #[cfg(test)]
        Pending,
    }

    type CredentialMockQueue = HashMap<String, VecDeque<MockCallResponse>>;
    static MOCK_CALLS: OnceLock<Mutex<VecDeque<MockCallResponse>>> =
        OnceLock::new();
    static CREDENTIAL_MOCK_CALLS: OnceLock<Mutex<CredentialMockQueue>> =
        OnceLock::new();

    fn queue() -> &'static Mutex<VecDeque<MockCallResponse>> {
        MOCK_CALLS.get_or_init(|| Mutex::new(VecDeque::new()))
    }

    fn credential_queues() -> &'static Mutex<CredentialMockQueue> {
        CREDENTIAL_MOCK_CALLS.get_or_init(|| Mutex::new(HashMap::new()))
    }

    pub fn push(response: Result<Response, ApiError>) {
        queue()
            .lock()
            .expect("mock call queue")
            .push_back(MockCallResponse::Immediate(response));
    }

    #[cfg(test)]
    pub fn push_delayed(
        delay: std::time::Duration,
        response: Result<Response, ApiError>,
    ) {
        queue()
            .lock()
            .expect("mock call queue")
            .push_back(MockCallResponse::Delayed(delay, response));
    }

    #[cfg(test)]
    pub fn push_pending() {
        queue()
            .lock()
            .expect("mock call queue")
            .push_back(MockCallResponse::Pending);
    }

    /// Deprecated: use [`install_upstream_mock`] from `gateway_tests`.
    pub fn push_for_credential(
        credential_id: &str,
        response: Result<Response, ApiError>,
    ) {
        credential_queues()
            .lock()
            .expect("credential mock mutex")
            .entry(credential_id.to_string())
            .or_default()
            .push_back(MockCallResponse::Immediate(response));
    }

    pub fn install_upstream_mock(script: gateway_tests::UpstreamMockScript) {
        gateway_tests::install_upstream_mock(script);
    }

    pub fn pop(credential_id: &str, model: &str) -> Option<MockCallResponse> {
        if let Some(response) =
            gateway_tests::pop_upstream_response(credential_id, model)
        {
            return Some(MockCallResponse::Immediate(Ok(response)));
        }
        if let Some(response) = credential_queues()
            .lock()
            .expect("credential mock mutex")
            .get_mut(credential_id)
            .and_then(VecDeque::pop_front)
        {
            return Some(response);
        }
        queue().lock().expect("mock call queue").pop_front()
    }

    pub fn clear() {
        gateway_tests::clear_upstream_mocks();
        if let Some(queue) = MOCK_CALLS.get() {
            queue.lock().expect("mock call queue").clear();
        }
        if let Some(map) = CREDENTIAL_MOCK_CALLS.get() {
            map.lock().expect("credential mock mutex").clear();
        }
    }

    #[cfg(test)]
    pub fn queued_len() -> usize {
        queue().lock().expect("mock call queue").len()
    }

    #[cfg(test)]
    mod tests {
        use axum_core::body::Body;
        use http::StatusCode;

        use super::*;

        fn ok() -> Response {
            http::Response::builder()
                .status(StatusCode::OK)
                .body(Body::from("ok"))
                .unwrap()
        }

        fn unwrap_mock_response(mock: MockCallResponse) -> Response {
            match mock {
                MockCallResponse::Immediate(response) => response.expect("ok"),
                MockCallResponse::Delayed(_, response) => response.expect("ok"),
                MockCallResponse::Pending => {
                    panic!("pending mock response has no value")
                }
            }
        }

        #[test]
        fn script_precedes_legacy_fifo() {
            clear();
            push_for_credential("gemini-free", Ok(ok()));
            install_upstream_mock(
                gateway_tests::UpstreamMockScript::new().binding(
                    "gemini-free",
                    "gemini-3.1-flash-lite",
                    vec![gateway_tests::upstream::ok_chat_completion],
                ),
            );
            let response = unwrap_mock_response(
                pop("gemini-free", "gemini-3.1-flash-lite").expect("script"),
            );
            assert_eq!(response.status(), StatusCode::OK);
            let legacy = unwrap_mock_response(
                pop("gemini-free", "other-model")
                    .expect("legacy fifo for same credential"),
            );
            assert_eq!(legacy.status(), StatusCode::OK);
        }
    }
}

#[cfg(feature = "testing")]
pub use test_hooks::{
    clear as clear_test_call_responses, install_upstream_mock,
    push as push_test_call_response,
    push_for_credential as push_test_call_response_for_credential,
};
#[cfg(all(test, feature = "testing"))]
pub use test_hooks::{
    push_delayed as push_delayed_test_call_response,
    push_pending as push_pending_test_call_response,
    queued_len as test_call_response_queue_len,
};

pub(super) async fn call_candidate(
    candidate: &BudgetCandidate,
    req: Request,
) -> Result<Response, ApiError> {
    #[cfg(feature = "testing")]
    if let Some(response) = test_hooks::pop(
        candidate.credential_id.as_str(),
        &candidate.capability.model.to_string(),
    ) {
        match response {
            test_hooks::MockCallResponse::Immediate(response) => {
                return response;
            }
            #[cfg(test)]
            test_hooks::MockCallResponse::Delayed(delay, response) => {
                tokio::time::sleep(delay).await;
                return response;
            }
            #[cfg(test)]
            test_hooks::MockCallResponse::Pending => {
                std::future::pending::<()>().await;
                unreachable!("pending mock call should not resolve");
            }
        }
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
