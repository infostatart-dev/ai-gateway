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
    types::{
        extensions::CallerRequestContext, request::Request, response::Response,
    },
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

        let pool =
            this.ordered_candidates(&requirements, source_model.as_ref())?;

        let caller = parts
            .extensions
            .get::<CallerRequestContext>()
            .cloned()
            .unwrap_or_else(|| {
                let (work_unit_id, work_unit_source) =
                    crate::middleware::caller_context::resolve_work_unit(
                        &http::HeaderMap::new(),
                    );
                CallerRequestContext {
                    agent_name:
                        crate::middleware::caller_context::DEFAULT_AGENT_NAME
                            .to_string(),
                    work_unit_id: Some(work_unit_id),
                    work_unit_source,
                }
            });
        let estimated_tokens = requirements.min_context_tokens.unwrap_or(0);
        let plan = super::plan::plan_route_chain(
            &this,
            pool.clone(),
            &requirements,
            routing_intent,
            &caller,
            this.app_state.credential_health(),
            this.app_state.route_memory(),
            estimated_tokens,
            &std::collections::HashSet::new(),
        )
        .await;
        if plan.chain.is_empty() {
            return Err(ApiError::Internal(InternalError::ProviderNotFound));
        }
        let candidates = plan.chain;
        let mut parts = parts;
        parts
            .extensions
            .insert(crate::types::extensions::RoutePlanContext {
                caller: caller.clone(),
                full_pool: pool,
                estimated_tokens,
                route_memory_hit: plan.route_memory_hit,
                planned_hops: plan.planned_hops,
                source_model: source_model.as_ref().map(ToString::to_string),
                json_schema_required: requirements.json_schema_required,
                replay: plan.replay,
            });

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
