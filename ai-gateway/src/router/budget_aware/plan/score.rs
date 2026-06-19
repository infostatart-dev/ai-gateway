use super::super::types::BudgetCandidate;
use crate::{
    config::{
        cost_class::CostClass,
        model_ladder::{LadderBand, ModelLadderRegistry},
    },
    router::budget_aware::{CredentialHealthRegistry, memory::RouteBinding},
    types::extensions::ReplayScoreBreakdown,
};

pub struct ScoreInput<'a> {
    pub candidate: &'a BudgetCandidate,
    pub health: &'a CredentialHealthRegistry,
    pub headroom: f64,
    pub affinity: bool,
    pub hash_bias: f64,
    pub cooldown_secs: f64,
}

const W_HEALTH: f64 = 0.30;
const W_HEADROOM: f64 = 0.25;
const W_AFFINITY: f64 = 0.20;
const W_HASH: f64 = 0.10;
const W_COST: f64 = 0.10;
const W_COOLDOWN: f64 = 0.15;
const W_LADDER: f64 = 0.05;

#[allow(dead_code)]
#[must_use]
pub fn score(input: &ScoreInput<'_>) -> f64 {
    score_breakdown(input).score
}

#[must_use]
pub fn score_breakdown(input: &ScoreInput<'_>) -> ReplayScoreBreakdown {
    let h_success = input.health.success_rate(
        &input.candidate.capability.provider,
        &input.candidate.credential_id,
    );
    let quota_capacity = input.headroom;
    let q_cooldown_secs = input.cooldown_secs;
    let m_affinity = if input.affinity { 1.0 } else { 0.0 };
    let hash_bias = input.hash_bias;
    let l_band = ladder_band_index(
        &input.candidate.capability.provider,
        &input.candidate.credential_tier,
        &input.candidate.capability.model.to_string(),
    );
    let cost_class = cost_class_label(input.candidate.credential_cost_class);
    let cost = f64::from(input.candidate.credential_cost_class.rank_base());
    let score = W_HEALTH * h_success
        + W_HEADROOM * quota_capacity
        + W_AFFINITY * m_affinity
        + W_HASH * hash_bias
        + W_COST * (1.0 / (1.0 + cost))
        - W_COOLDOWN * norm_cooldown(q_cooldown_secs)
        - W_LADDER * f64::from(l_band);
    ReplayScoreBreakdown {
        score,
        h_success,
        quota_capacity,
        q_cooldown_secs,
        m_affinity,
        hash_bias,
        l_band,
        cost_class,
    }
}

#[must_use]
pub fn hash_bias(
    agent_name: &str,
    work_unit_id: &str,
    credential_id: &str,
) -> f64 {
    let hash = stable_hash(agent_name, work_unit_id, credential_id);
    f64::from((hash % 1000) as u32) / 1000.0
}

#[must_use]
pub fn binding_matches(
    candidate: &BudgetCandidate,
    binding: &RouteBinding,
) -> bool {
    candidate.credential_id == binding.credential_id
        && candidate.capability.model.to_string() == binding.model
}

fn norm_cooldown(secs: f64) -> f64 {
    secs / (secs + 60.0)
}

fn cost_class_label(class: CostClass) -> String {
    match class {
        CostClass::Free => "free".to_string(),
        CostClass::Paid => "paid".to_string(),
        CostClass::PaidBrowser => "paid-browser".to_string(),
    }
}

fn ladder_band_index(
    provider: &crate::types::provider::InferenceProvider,
    tier: &str,
    model: &str,
) -> u16 {
    let ladders = ModelLadderRegistry::default();
    ladders
        .position(provider, tier, model)
        .map_or(2, |pos| match pos.band {
            LadderBand::Fast => 0,
            LadderBand::Capacity => 1,
            LadderBand::Stability => 2,
            LadderBand::Deprioritized => 3,
        })
}

#[must_use]
pub fn spread_slot_index(
    agent_name: &str,
    work_unit_id: &str,
    credential_id: &str,
    slots: usize,
) -> usize {
    if slots == 0 {
        return 0;
    }
    match usize::try_from(stable_hash(agent_name, work_unit_id, credential_id))
    {
        Ok(hash) => hash % slots,
        Err(_) => 0,
    }
}

fn stable_hash(agent: &str, work_unit: &str, credential: &str) -> u64 {
    use std::{
        collections::hash_map::DefaultHasher,
        hash::{Hash, Hasher},
    };
    let mut hasher = DefaultHasher::new();
    agent.hash(&mut hasher);
    work_unit.hash(&mut hasher);
    credential.hash(&mut hasher);
    hasher.finish()
}
