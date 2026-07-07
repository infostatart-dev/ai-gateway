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
    memory::{
        GatewayRouteMemory, RouteBindingPreference, RouteMemoryKey,
        RouteStreamMode,
    },
    types::{BudgetAwareRouter, BudgetCandidate},
};
use crate::{
    router::{capability::RequestRequirements, intent::RoutingIntent},
    types::extensions::CallerRequestContext,
};

pub struct PlanContext<'a> {
    pub caller: &'a CallerRequestContext,
    pub memory_key: &'a RouteMemoryKey,
    pub health: &'a CredentialHealthRegistry,
    pub snapshot: &'a Snapshot,
    pub requirements: &'a RequestRequirements,
    pub intent: Option<RoutingIntent>,
    pub now: Instant,
    pub pool: &'a [BudgetCandidate],
}

pub struct PlanResult {
    pub chain: Vec<BudgetCandidate>,
    pub memory_key: RouteMemoryKey,
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
    memory: &GatewayRouteMemory,
    estimated_tokens: u32,
    exclude: &HashSet<(String, String)>,
    source_model: Option<&str>,
    stream: RouteStreamMode,
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
    let memory_key = RouteMemoryKey::for_route_class(
        &router.router_id,
        router.endpoint_type,
        requirements,
        intent,
        source_model,
        stream,
    );
    let ctx = PlanContext {
        caller,
        memory_key: &memory_key,
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
            memory_key: memory_key.clone(),
            route_memory_hit: false,
            planned_hops: 0,
            replay: None,
        };
    }

    let memory_bindings = memory.preferred(ctx.memory_key).await;
    let route_memory_hit = memory_bindings.iter().any(|binding| {
        survivors
            .iter()
            .any(|c| score::binding_matches(c, &binding.binding))
            && binding_viable(&ctx, binding)
    });
    let chain = build_chain(router, &ctx, &survivors, &memory_bindings);
    let replay = chain.first().map(|hop0| {
        capture_replay(&ctx, router, &survivors, &memory_bindings, hop0, &chain)
    });
    PlanResult {
        planned_hops: u32::try_from(chain.len()).unwrap_or(u32::MAX),
        chain,
        memory_key: memory_key.clone(),
        route_memory_hit,
        replay,
    }
}

fn binding_viable(
    ctx: &PlanContext<'_>,
    binding: &RouteBindingPreference,
) -> bool {
    ctx.snapshot.headroom_score(
        binding.binding.credential_id.as_str(),
        &binding.binding.model,
    ) > 0.0
}
