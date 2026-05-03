use futures::future::BoxFuture;
use http_body_util::BodyExt;

use super::{failover_loop, types::BudgetAwareRouter};
use crate::{
    error::{api::ApiError, internal::InternalError},
    router::capability::{extract_requirements, extract_source_model},
    types::{request::Request, response::Response},
};

pub(super) fn budget_aware_call(
    this: BudgetAwareRouter,
    req: Request,
) -> BoxFuture<'static, Result<Response, ApiError>> {
    Box::pin(async move {
        let (parts, body) = req.into_parts();
        let body_bytes = body
            .collect()
            .await
            .map_err(InternalError::CollectBodyError)?
            .to_bytes();
        let requirements = extract_requirements(&body_bytes);
        let source_model = extract_source_model(&body_bytes);
        let candidates =
            this.ordered_candidates(&requirements, source_model.as_ref())?;

        failover_loop::run_failover_candidates(
            this, parts, body_bytes, candidates,
        )
        .await
    })
}
