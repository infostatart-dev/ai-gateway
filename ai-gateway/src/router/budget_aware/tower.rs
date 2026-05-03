use std::task::{Context, Poll};

use futures::future::BoxFuture;
use tower::Service;

use super::{dispatch, types::BudgetAwareRouter};
use crate::{
    error::api::ApiError,
    types::{request::Request, response::Response},
};

impl Service<Request> for BudgetAwareRouter {
    type Response = Response;
    type Error = ApiError;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(
        &mut self,
        _cx: &mut Context<'_>,
    ) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Request) -> Self::Future {
        dispatch::budget_aware_call(self.clone(), req)
    }
}
