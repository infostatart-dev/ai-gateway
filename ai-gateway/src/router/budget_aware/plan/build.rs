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

    append_intra_slot_ladder(router, ctx, &scored, &mut plan, &mut used);
    for (_, candidate) in &scored {
        if plan.len() >= MAX_PLAN_HOPS {
            break;
        }
        let key = plan_key(candidate);
        if used.insert(key) {
            plan.push(candidate.clone());
        }
    }
    plan.truncate(MAX_PLAN_HOPS);
    apply_spread(&mut plan, ctx.caller);
    plan
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
    let mut slots: Vec<ProviderCredentialId> = scored
        .iter()
        .map(|(_, c)| c.credential_id.clone())
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    slots.sort_by_key(std::string::ToString::to_string);

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
        if candidate.capability.intent_tier < floor {
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

fn apply_spread(plan: &mut [BudgetCandidate], caller: &CallerRequestContext) {
    let Some(work_unit) = caller.work_unit_id.as_deref() else {
        return;
    };
    if plan.is_empty() {
        return;
    }
    let anchor = &plan[0];
    let anchor_model = anchor.capability.model.to_string();
    let anchor_provider = &anchor.capability.provider;
    let peer_positions: Vec<usize> = plan
        .iter()
        .enumerate()
        .filter(|(_, candidate)| {
            candidate.capability.provider == *anchor_provider
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
