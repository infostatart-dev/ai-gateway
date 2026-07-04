use std::collections::HashSet;

use bytes::Bytes;
use http::request::Parts;

use crate::{
    error::api::ApiError,
    router::budget_aware::plan::plan_route_chain,
    tests::routing::{
        BudgetAwareRouter, BudgetCandidate, CallerRequestContext,
        RequestRequirements, Response, RoutePlanContext, RoutingIntent,
        router_app_state, run_failover_candidates,
    },
};

pub struct PlannedResponse {
    pub response: Response,
    pub planned_hops: u32,
    pub route_memory_hit: bool,
}

pub async fn run_planned_failover(
    router: BudgetAwareRouter,
    parts: Parts,
    body: Bytes,
    pool: Vec<BudgetCandidate>,
    requirements: RequestRequirements,
    routing_intent: Option<RoutingIntent>,
) -> Result<PlannedResponse, ApiError> {
    let caller = parts
        .extensions
        .get::<CallerRequestContext>()
        .cloned()
        .unwrap_or_else(|| {
            let (work_unit_id, work_unit_source) =
                crate::middleware::caller_context::resolve_work_unit(
                    &parts.headers,
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
    let plan = plan_route_chain(
        &router,
        pool.clone(),
        &requirements,
        routing_intent,
        &caller,
        router_app_state(&router).credential_health(),
        router_app_state(&router).route_memory(),
        estimated_tokens,
        &HashSet::new(),
    )
    .await;
    if plan.chain.is_empty() {
        if !pool.is_empty() {
            return Ok(PlannedResponse {
                response: crate::router::budget_aware::route_exhausted_response(
                    std::time::Duration::from_secs(1),
                ),
                planned_hops: 0,
                route_memory_hit: false,
            });
        }
        return Err(ApiError::Internal(
            crate::error::internal::InternalError::ProviderNotFound,
        ));
    }
    let full_pool = pool;
    let candidates = plan.chain;
    let mut parts = parts;
    parts.extensions.insert(RoutePlanContext {
        caller,
        full_pool,
        estimated_tokens,
        route_memory_hit: plan.route_memory_hit,
        planned_hops: plan.planned_hops,
        source_model: None,
        json_schema_required: requirements.json_schema_required,
        replay: plan.replay,
    });
    let response = run_failover_candidates(
        router,
        parts,
        body,
        candidates,
        requirements,
        routing_intent,
    )
    .await?;
    Ok(PlannedResponse {
        response,
        planned_hops: plan.planned_hops,
        route_memory_hit: plan.route_memory_hit,
    })
}
