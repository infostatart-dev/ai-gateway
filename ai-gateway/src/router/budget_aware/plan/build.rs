use std::{collections::HashSet, time::Duration};

use super::{PlanContext, replay::rank_survivors, score::binding_matches};
use crate::{
    config::{
        credentials::ProviderCredentialId,
        model_ladder::{LadderBand, ModelLadderRegistry},
    },
    router::{
        budget_aware::{
            memory::RouteBinding,
            types::{BudgetAwareRouter, BudgetCandidate},
        },
        intent::{IntentTier, RoutingIntent},
        provider_attempt::{
            ModelCooldownKey, lock_credential_states, lock_model_states,
        },
    },
    types::{extensions::CallerRequestContext, provider::InferenceProvider},
};

pub const MAX_PLAN_HOPS: usize = 7;
const STRATEGIC_FALLBACKS: [(&str, usize); 4] = [
    ("deepseek-web", 2),
    ("chatgpt-web", 1),
    ("longcat", 1),
    ("vllm", 1),
];

pub fn build_chain(
    router: &BudgetAwareRouter,
    ctx: &PlanContext<'_>,
    survivors: &[BudgetCandidate],
    memory_binding: Option<&RouteBinding>,
) -> Vec<BudgetCandidate> {
    if survivors.is_empty() {
        return Vec::new();
    }
    let scored: Vec<(f64, BudgetCandidate)> =
        rank_survivors(router, ctx, survivors, memory_binding)
            .into_iter()
            .map(|entry| (entry.breakdown.score, entry.candidate))
            .collect();

    let mut plan = Vec::new();
    let mut used = HashSet::new();
    if let Some(binding) = memory_binding
        && let Some((_, candidate)) =
            scored.iter().find(|(_, c)| binding_matches(c, binding))
    {
        let key = plan_key(candidate);
        if used.insert(key) {
            plan.push(candidate.clone());
        }
    }

    append_strategic_fallback_hops(&scored, &mut plan, &mut used);
    append_intra_slot_ladder(router, ctx, &scored, &mut plan, &mut used);
    append_scored_hops(&scored, &mut plan, &mut used);
    plan.truncate(MAX_PLAN_HOPS);
    apply_spread(&mut plan, ctx.caller, &scored);
    plan
}

fn append_strategic_fallback_hops(
    scored: &[(f64, BudgetCandidate)],
    plan: &mut Vec<BudgetCandidate>,
    used: &mut HashSet<(ProviderCredentialId, String)>,
) {
    for (provider_name, max_slots) in STRATEGIC_FALLBACKS {
        let mut added = 0;
        for (_, candidate) in scored {
            if plan.len() >= MAX_PLAN_HOPS || added >= max_slots {
                break;
            }
            if !is_named_provider(candidate, provider_name) {
                continue;
            }
            let key = plan_key(candidate);
            if used.insert(key) {
                plan.push(candidate.clone());
                added += 1;
            }
        }
    }
}

fn is_named_provider(candidate: &BudgetCandidate, provider_name: &str) -> bool {
    matches!(
        &candidate.capability.provider,
        InferenceProvider::Named(name) if name == provider_name
    )
}

fn append_scored_hops(
    scored: &[(f64, BudgetCandidate)],
    plan: &mut Vec<BudgetCandidate>,
    used: &mut HashSet<(ProviderCredentialId, String)>,
) {
    let Some(anchor) = anchor_credential_for_ladder(scored, plan) else {
        append_scored_hops_pass(scored, plan, used, |_| true);
        return;
    };
    let anchor_provider = scored
        .iter()
        .find(|(_, candidate)| candidate.credential_id == anchor)
        .map(|(_, candidate)| candidate.capability.provider.clone());
    let Some(anchor_provider) = anchor_provider else {
        append_scored_hops_pass(scored, plan, used, |_| true);
        return;
    };
    append_scored_hops_pass(scored, plan, used, |candidate| {
        candidate.capability.provider != anchor_provider
    });
    append_scored_hops_pass(scored, plan, used, |_| true);
}

fn append_scored_hops_pass(
    scored: &[(f64, BudgetCandidate)],
    plan: &mut Vec<BudgetCandidate>,
    used: &mut HashSet<(ProviderCredentialId, String)>,
    include: impl Fn(&BudgetCandidate) -> bool,
) {
    for (_, candidate) in scored {
        if plan.len() >= MAX_PLAN_HOPS {
            break;
        }
        if !include(candidate) {
            continue;
        }
        let key = plan_key(candidate);
        if used.insert(key) {
            plan.push(candidate.clone());
        }
    }
}

fn anchor_credential_for_ladder(
    scored: &[(f64, BudgetCandidate)],
    plan: &[BudgetCandidate],
) -> Option<ProviderCredentialId> {
    plan.first()
        .map(|candidate| candidate.credential_id.clone())
        .or_else(|| {
            scored
                .first()
                .map(|(_, candidate)| candidate.credential_id.clone())
        })
}

fn append_intra_slot_ladder(
    router: &BudgetAwareRouter,
    ctx: &PlanContext<'_>,
    scored: &[(f64, BudgetCandidate)],
    plan: &mut Vec<BudgetCandidate>,
    used: &mut HashSet<(ProviderCredentialId, String)>,
) {
    let ladders = ModelLadderRegistry::default();
    let floor = ctx
        .intent
        .map(|intent| intent.effective_floor(ctx.requirements))
        .or(ctx.requirements.preferred_intent_tier);
    let Some(anchor) = anchor_credential_for_ladder(scored, plan) else {
        return;
    };
    // Ladder escalation is per primary slot only; other accounts enter via
    // scored cross-provider hops so multi-account pools cannot monopolize
    // MAX_PLAN_HOPS.
    let slots = [anchor];

    for slot in slots {
        if plan.len() >= MAX_PLAN_HOPS {
            break;
        }
        let slot_candidates: Vec<_> = scored
            .iter()
            .filter(|(_, c)| c.credential_id == slot)
            .map(|(_, c)| c)
            .collect();
        if slot_candidates.is_empty() {
            continue;
        }
        let provider = &slot_candidates[0].capability.provider;
        let tier = &slot_candidates[0].credential_tier;
        for band in [
            LadderBand::Fast,
            LadderBand::Capacity,
            LadderBand::Stability,
        ] {
            if plan.len() >= MAX_PLAN_HOPS {
                break;
            }
            let models = ladders.models_in_band(provider, tier, band);
            for model in models {
                if let Some(candidate) = slot_candidates.iter().find(|c| {
                    c.capability.model.to_string() == model
                        && feasible_candidate(router, ctx, c, floor)
                }) {
                    let key = plan_key(candidate);
                    if used.insert(key) {
                        plan.push((*candidate).clone());
                    }
                }
            }
        }
    }
}

fn feasible_candidate(
    router: &BudgetAwareRouter,
    ctx: &PlanContext<'_>,
    candidate: &BudgetCandidate,
    floor: Option<IntentTier>,
) -> bool {
    feasible_for_plan(router, ctx, candidate, ctx.intent)
        && floor.is_none_or(|floor| candidate.capability.intent_tier >= floor)
}

#[must_use]
pub fn feasible_for_plan(
    _router: &BudgetAwareRouter,
    ctx: &PlanContext<'_>,
    candidate: &BudgetCandidate,
    intent: Option<RoutingIntent>,
) -> bool {
    let model = candidate.capability.model.to_string();
    if ctx.health.is_circuit_open(
        &candidate.capability.provider,
        &candidate.credential_id,
        ctx.now,
    ) {
        return false;
    }
    if ctx
        .snapshot
        .headroom_score(candidate.credential_id.as_str(), &model)
        <= 0.0
    {
        return false;
    }
    if ctx.health.credential_zero_success_dead(
        &candidate.capability.provider,
        &candidate.credential_id,
        ctx.now,
    ) {
        return false;
    }
    if let Some(intent) = intent {
        let floor = intent.effective_floor(ctx.requirements);
        if candidate.capability.intent_tier < floor
            && !is_strategic_fallback_provider(candidate)
        {
            return false;
        }
    }
    if deprioritized_while_gemini_stability_available(candidate, ctx) {
        return false;
    }
    true
}

fn deprioritized_while_gemini_stability_available(
    candidate: &BudgetCandidate,
    ctx: &PlanContext<'_>,
) -> bool {
    let ladders = ModelLadderRegistry::default();
    let pos = ladders.position(
        &candidate.capability.provider,
        &candidate.credential_tier,
        &candidate.capability.model.to_string(),
    );
    if !matches!(pos.map(|p| p.band), Some(LadderBand::Deprioritized)) {
        return false;
    }
    let stability_models = ladders.models_in_band(
        &InferenceProvider::GoogleGemini,
        "free",
        LadderBand::Stability,
    );
    ctx.pool.iter().any(|slot| {
        slot.capability.provider == InferenceProvider::GoogleGemini
            && stability_models.iter().any(|model| {
                slot.capability.model.to_string() == *model
                    && ctx
                        .snapshot
                        .headroom_score(slot.credential_id.as_str(), model)
                        > 0.0
            })
    })
}

fn is_strategic_fallback_provider(candidate: &BudgetCandidate) -> bool {
    STRATEGIC_FALLBACKS
        .iter()
        .any(|(provider_name, _)| is_named_provider(candidate, provider_name))
}

fn apply_spread(
    plan: &mut [BudgetCandidate],
    caller: &CallerRequestContext,
    scored: &[(f64, BudgetCandidate)],
) {
    let Some(work_unit) = caller.work_unit_id.as_deref() else {
        return;
    };
    if plan.is_empty() {
        return;
    }
    let anchor_model = plan[0].capability.model.to_string();
    let anchor_provider = plan[0].capability.provider.clone();
    let feasible_peers: Vec<BudgetCandidate> = scored
        .iter()
        .filter(|(_, candidate)| {
            candidate.capability.provider == anchor_provider
                && candidate.capability.model.to_string() == anchor_model
        })
        .map(|(_, candidate)| candidate.clone())
        .collect();
    if feasible_peers.is_empty() {
        return;
    }
    let mut spread_pool: Vec<_> = feasible_peers
        .iter()
        .map(|candidate| candidate.credential_id.to_string())
        .collect();
    spread_pool.sort();
    let idx = super::score::spread_pool_index(
        &caller.agent_name,
        work_unit,
        &spread_pool,
    );
    if let Some(candidate) = feasible_peers
        .iter()
        .find(|candidate| candidate.credential_id.as_str() == spread_pool[idx])
    {
        plan[0] = candidate.clone();
    }

    let peer_positions: Vec<usize> = plan
        .iter()
        .enumerate()
        .skip(1)
        .filter(|(_, candidate)| {
            candidate.capability.provider == anchor_provider
                && candidate.capability.model.to_string() == anchor_model
        })
        .map(|(index, _)| index)
        .collect();
    if peer_positions.len() <= 1 {
        return;
    }
    let peers: Vec<BudgetCandidate> = peer_positions
        .iter()
        .map(|index| plan[*index].clone())
        .collect();
    let idx = super::score::spread_slot_index(
        &caller.agent_name,
        work_unit,
        peers[0].credential_id.as_str(),
        peers.len(),
    );
    let rotated: Vec<_> =
        peers[idx..].iter().chain(&peers[..idx]).cloned().collect();
    for (position, candidate) in peer_positions.into_iter().zip(rotated) {
        plan[position] = candidate;
    }
}

fn plan_key(candidate: &BudgetCandidate) -> (ProviderCredentialId, String) {
    (
        candidate.credential_id.clone(),
        candidate.capability.model.to_string(),
    )
}

pub(crate) fn effective_cooldown_secs(
    router: &BudgetAwareRouter,
    candidate: &BudgetCandidate,
    now: std::time::Instant,
    pacing_wait: Duration,
) -> f64 {
    let slot_model = model_and_slot_cooldown_secs(router, candidate, now);
    slot_model.max(pacing_wait.as_secs_f64())
}

fn model_and_slot_cooldown_secs(
    router: &BudgetAwareRouter,
    candidate: &BudgetCandidate,
    now: std::time::Instant,
) -> f64 {
    let model_key = ModelCooldownKey {
        credential_id: candidate.credential_id.clone(),
        model: candidate.capability.model.to_string(),
    };
    let model_states = lock_model_states(&router.model_states);
    let model_remaining = model_states
        .get(&model_key)
        .and_then(|state| state.cooldown_until)
        .and_then(|until| until.checked_duration_since(now));
    drop(model_states);
    let states = lock_credential_states(&router.states);
    let slot_remaining = states
        .get(&candidate.credential_id)
        .and_then(|state| state.cooldown_until)
        .and_then(|until| until.checked_duration_since(now));
    match (slot_remaining, model_remaining) {
        (Some(a), Some(b)) => a.max(b).as_secs_f64(),
        (Some(a), None) => a.as_secs_f64(),
        (None, Some(b)) => b.as_secs_f64(),
        (None, None) => 0.0,
    }
}
