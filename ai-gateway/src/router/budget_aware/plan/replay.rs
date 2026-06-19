use super::{
    PlanContext,
    build::effective_cooldown_secs,
    score::{ScoreInput, binding_matches, hash_bias, score_breakdown},
};
use crate::{
    config::credentials::ProviderCredentialId,
    router::budget_aware::{
        memory::RouteBinding,
        types::{BudgetAwareRouter, BudgetCandidate},
    },
    types::extensions::{PlanReplaySnapshot, ReplayAlternative},
};

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
            let model = candidate.capability.model.to_string();
            let headroom = ctx
                .snapshot
                .headroom_score(candidate.credential_id.as_str(), &model);
            let affinity = memory_binding
                .is_some_and(|binding| binding_matches(candidate, binding));
            let hash =
                ctx.caller.work_unit_id.as_deref().map_or(0.0, |work_unit| {
                    hash_bias(
                        &ctx.caller.agent_name,
                        work_unit,
                        candidate.credential_id.as_str(),
                    )
                });
            let cooldown_secs = planner_cooldown_secs(router, ctx, candidate);
            let breakdown = score_breakdown(&ScoreInput {
                candidate,
                health: ctx.health,
                headroom,
                affinity,
                hash_bias: hash,
                cooldown_secs,
            });
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
                score_breakdown(&ScoreInput {
                    candidate: hop0,
                    health: ctx.health,
                    headroom: ctx.snapshot.headroom_score(
                        hop0.credential_id.as_str(),
                        &hop0.capability.model.to_string(),
                    ),
                    affinity: false,
                    hash_bias: 0.0,
                    cooldown_secs: planner_cooldown_secs(router, ctx, hop0),
                })
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
    }
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
