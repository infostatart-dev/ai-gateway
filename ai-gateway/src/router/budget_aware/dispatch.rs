use futures::future::BoxFuture;
use http_body_util::BodyExt;
use serde_json::Value;

use super::{failover_loop, types::BudgetAwareRouter};
use crate::{
    error::{api::ApiError, internal::InternalError},
    router::{
        capability::{
            apply_payload_estimate, extract_requirements_from_value,
            extract_source_model_from_value,
        },
        token_estimate::{PayloadBudgetConfig, estimate_from_value},
    },
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

        let parsed: Option<Value> = serde_json::from_slice(&body_bytes).ok();
        let budget = PayloadBudgetConfig::default();
        let mut requirements = parsed
            .as_ref()
            .map(extract_requirements_from_value)
            .unwrap_or_default();
        if let Some(value) = parsed.as_ref()
            && let Some(estimate) = estimate_from_value(value, budget)
        {
            apply_payload_estimate(&mut requirements, estimate);
        }
        let source_model =
            parsed.as_ref().and_then(extract_source_model_from_value);
        let routing_intent = source_model
            .as_ref()
            .map(crate::router::intent::extract_routing_intent);

        let candidates =
            this.ordered_candidates(&requirements, source_model.as_ref())?;

        failover_loop::run_failover_candidates(
            this,
            parts,
            body_bytes,
            candidates,
            requirements,
            routing_intent,
        )
        .await
    })
}
