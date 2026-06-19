pub(crate) mod build;
pub(crate) mod replay;
pub(crate) mod score;
pub mod snapshot;

use std::{collections::HashSet, time::Instant};

use build::build_chain;
use replay::capture_replay;
use snapshot::QuotaSnapshot as Snapshot;

use super::{
    CredentialHealthRegistry,
    memory::{RouteBinding, WorkUnitRouteMemory},
    types::{BudgetAwareRouter, BudgetCandidate},
};
use crate::{
    router::{capability::RequestRequirements, intent::RoutingIntent},
    types::extensions::CallerRequestContext,
};

pub struct PlanContext<'a> {
    pub caller: &'a CallerRequestContext,
    pub health: &'a CredentialHealthRegistry,
    pub snapshot: &'a Snapshot,
    pub requirements: &'a RequestRequirements,
    pub intent: Option<RoutingIntent>,
    pub now: Instant,
    pub pool: &'a [BudgetCandidate],
}

pub struct PlanResult {
    pub chain: Vec<BudgetCandidate>,
    pub route_memory_hit: bool,
    pub planned_hops: u32,
    pub replay: Option<crate::types::extensions::PlanReplaySnapshot>,
}

#[allow(clippy::implicit_hasher)]
#[allow(clippy::too_many_arguments)]
pub async fn plan_route_chain(
    router: &BudgetAwareRouter,
    candidates: Vec<BudgetCandidate>,
    requirements: &RequestRequirements,
    intent: Option<RoutingIntent>,
    caller: &CallerRequestContext,
    health: &CredentialHealthRegistry,
    memory: &WorkUnitRouteMemory,
    estimated_tokens: u32,
    exclude: &HashSet<(String, String)>,
) -> PlanResult {
    let now = Instant::now();
    let snapshot = Snapshot::capture(
        router.app_state.upstream_pacing(),
        health,
        router,
        &candidates,
        estimated_tokens,
        router.max_cooldown_wait,
        now,
    )
    .await;
    let pool: Vec<_> = candidates
        .into_iter()
        .filter(|candidate| {
            let model = candidate.capability.model.to_string();
            let key = (candidate.credential_id.to_string(), model);
            !exclude.contains(&key)
        })
        .collect();
    let ctx = PlanContext {
        caller,
        health,
        snapshot: &snapshot,
        requirements,
        intent,
        now,
        pool: &pool,
    };
    let survivors: Vec<_> = pool
        .iter()
        .filter(|candidate| {
            build::feasible_for_plan(router, &ctx, candidate, intent)
        })
        .cloned()
        .collect();
    if survivors.is_empty() {
        return PlanResult {
            chain: Vec::new(),
            route_memory_hit: false,
            planned_hops: 0,
            replay: None,
        };
    }

    let memory_binding = if let Some(work_unit) = caller.work_unit_id.as_deref()
    {
        memory.get(&caller.agent_name, work_unit).await
    } else {
        None
    };
    let route_memory_hit = memory_binding.as_ref().is_some_and(|binding| {
        survivors.iter().any(|c| score::binding_matches(c, binding))
            && binding_viable(&ctx, binding)
    });
    let chain = build_chain(router, &ctx, &survivors, memory_binding.as_ref());
    let replay = chain.first().map(|hop0| {
        capture_replay(&ctx, router, &survivors, memory_binding.as_ref(), hop0)
    });
    PlanResult {
        planned_hops: u32::try_from(chain.len()).unwrap_or(u32::MAX),
        chain,
        route_memory_hit,
        replay,
    }
}

fn binding_viable(ctx: &PlanContext<'_>, binding: &RouteBinding) -> bool {
    ctx.snapshot
        .headroom_score(binding.credential_id.as_str(), &binding.model)
        > 0.0
}
