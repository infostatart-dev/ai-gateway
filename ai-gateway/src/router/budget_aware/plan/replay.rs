use std::collections::HashSet;

use super::{
    PlanContext,
    build::effective_cooldown_secs,
    score::{ScoreInput, binding_matches, hash_bias, score_breakdown},
};
use crate::{
    config::credentials::ProviderCredentialId,
    router::{
        budget_aware::{
            memory::RouteBinding,
            types::{BudgetAwareRouter, BudgetCandidate},
        },
        quota_admission::BlockedReason,
    },
    types::extensions::{
        PlanReplaySnapshot, ReplayAlternative, ReplayQuotaExcluded,
    },
};

const MAX_QUOTA_EXCLUDED: usize = 8;

pub struct ScoredCandidate {
    pub breakdown: crate::types::extensions::ReplayScoreBreakdown,
    pub candidate: BudgetCandidate,
}

pub fn rank_survivors(
    router: &BudgetAwareRouter,
    ctx: &PlanContext<'_>,
    survivors: &[BudgetCandidate],
    memory_binding: Option<&RouteBinding>,
) -> Vec<ScoredCandidate> {
    let mut scored: Vec<_> = survivors
        .iter()
        .map(|candidate| {
            let breakdown = score_breakdown(&score_input_for_candidate(
                router,
                ctx,
                candidate,
                memory_binding,
            ));
            ScoredCandidate {
                breakdown,
                candidate: candidate.clone(),
            }
        })
        .collect();
    scored.sort_by(|left, right| {
        right
            .breakdown
            .score
            .partial_cmp(&left.breakdown.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    scored
}

#[must_use]
pub fn capture_replay(
    ctx: &PlanContext<'_>,
    router: &BudgetAwareRouter,
    survivors: &[BudgetCandidate],
    memory_binding: Option<&RouteBinding>,
    hop0: &BudgetCandidate,
) -> PlanReplaySnapshot {
    let scored = rank_survivors(router, ctx, survivors, memory_binding);
    let hop_key = plan_key(hop0);
    let winner = scored
        .iter()
        .find(|entry| plan_key(&entry.candidate) == hop_key)
        .map_or_else(
            || {
                score_breakdown(&score_input_for_candidate(
                    router, ctx, hop0, None,
                ))
            },
            |entry| entry.breakdown.clone(),
        );
    let top_alternatives = scored
        .iter()
        .filter(|entry| plan_key(&entry.candidate) != hop_key)
        .take(3)
        .map(|entry| ReplayAlternative {
            credential: entry.candidate.credential_id.to_string(),
            model: entry.candidate.capability.model.to_string(),
            score: entry.breakdown.score,
        })
        .collect();
    PlanReplaySnapshot {
        plan_snapshot_ts: ctx.snapshot.captured_at().to_rfc3339(),
        winner_credential: hop0.credential_id.to_string(),
        winner_model: hop0.capability.model.to_string(),
        winner,
        top_alternatives,
        quota_excluded: capture_quota_excluded(ctx, ctx.pool),
    }
}

#[must_use]
pub fn capture_quota_excluded(
    ctx: &PlanContext<'_>,
    pool: &[BudgetCandidate],
) -> Vec<ReplayQuotaExcluded> {
    let mut seen = HashSet::new();
    let mut excluded = Vec::new();
    for candidate in pool {
        if excluded.len() >= MAX_QUOTA_EXCLUDED {
            break;
        }
        if !is_quota_plan_exclusion(ctx, candidate) {
            continue;
        }
        let model = candidate.capability.model.to_string();
        let key = (candidate.credential_id.to_string(), model.clone());
        if !seen.insert(key) {
            continue;
        }
        let credential = candidate.credential_id.as_str();
        let blocked_reason = ctx.snapshot.blocked_reason(credential, &model);
        if blocked_reason == BlockedReason::None {
            continue;
        }
        excluded.push(ReplayQuotaExcluded {
            credential: candidate.credential_id.to_string(),
            model: model.clone(),
            blocked_reason,
            next_available_at: ctx
                .snapshot
                .next_available_at(credential, &model)
                .map(|instant| instant.to_rfc3339()),
            quota_capacity: 0.0,
        });
    }
    excluded
}

fn is_quota_plan_exclusion(
    ctx: &PlanContext<'_>,
    candidate: &BudgetCandidate,
) -> bool {
    let model = candidate.capability.model.to_string();
    if ctx.health.is_circuit_open(
        &candidate.capability.provider,
        &candidate.credential_id,
        ctx.now,
    ) {
        return false;
    }
    if ctx.health.credential_zero_success_dead(
        &candidate.capability.provider,
        &candidate.credential_id,
        ctx.now,
    ) {
        return false;
    }
    ctx.snapshot
        .headroom_score(candidate.credential_id.as_str(), &model)
        <= 0.0
}

fn score_input_for_candidate<'a>(
    router: &BudgetAwareRouter,
    ctx: &'a PlanContext<'_>,
    candidate: &'a BudgetCandidate,
    memory_binding: Option<&RouteBinding>,
) -> ScoreInput<'a> {
    let model = candidate.capability.model.to_string();
    let credential = candidate.credential_id.as_str();
    let headroom = ctx.snapshot.headroom_score(credential, &model);
    let (quota_blocked_reason, quota_next_available_at) =
        quota_block_fields(ctx, credential, &model, headroom);
    let affinity = memory_binding
        .is_some_and(|binding| binding_matches(candidate, binding));
    let hash = ctx.caller.work_unit_id.as_deref().map_or(0.0, |work_unit| {
        hash_bias(&ctx.caller.agent_name, work_unit, credential)
    });
    ScoreInput {
        candidate,
        health: ctx.health,
        headroom,
        affinity,
        hash_bias: hash,
        cooldown_secs: planner_cooldown_secs(router, ctx, candidate),
        quota_blocked_reason,
        quota_next_available_at,
    }
}

fn quota_block_fields(
    ctx: &PlanContext<'_>,
    credential_id: &str,
    model: &str,
    headroom: f64,
) -> (Option<BlockedReason>, Option<String>) {
    if headroom > 0.0 {
        return (None, None);
    }
    let reason = ctx.snapshot.blocked_reason(credential_id, model);
    if reason == BlockedReason::None {
        return (None, None);
    }
    (
        Some(reason),
        ctx.snapshot
            .next_available_at(credential_id, model)
            .map(|instant| instant.to_rfc3339()),
    )
}

fn planner_cooldown_secs(
    router: &BudgetAwareRouter,
    ctx: &PlanContext<'_>,
    candidate: &BudgetCandidate,
) -> f64 {
    let model = candidate.capability.model.to_string();
    let pacing = ctx
        .snapshot
        .next_wait(candidate.credential_id.as_str(), &model);
    effective_cooldown_secs(router, candidate, ctx.now, pacing)
}

fn plan_key(candidate: &BudgetCandidate) -> (ProviderCredentialId, String) {
    (
        candidate.credential_id.clone(),
        candidate.capability.model.to_string(),
    )
}
