use super::super::types::BudgetCandidate;
use crate::{
    config::{
        cost_class::CostClass,
        model_ladder::{LadderBand, ModelLadderRegistry},
    },
    router::{
        budget_aware::{CredentialHealthRegistry, memory::RouteBinding},
        quota_admission::BlockedReason,
    },
    types::extensions::ReplayScoreBreakdown,
};

pub struct ScoreInput<'a> {
    pub candidate: &'a BudgetCandidate,
    pub health: &'a CredentialHealthRegistry,
    pub headroom: f64,
    pub route_preference: f64,
    pub client_affinity: f64,
    pub cooldown_secs: f64,
    pub quota_blocked_reason: Option<BlockedReason>,
    pub quota_next_available_at: Option<String>,
}

const W_HEALTH: f64 = 0.30;
const W_HEADROOM: f64 = 0.25;
const W_ROUTE_PREFERENCE: f64 = 0.16;
const W_CLIENT_AFFINITY: f64 = 0.04;
const W_COST: f64 = 0.10;
const W_COOLDOWN: f64 = 0.15;
const W_LADDER: f64 = 0.05;

#[must_use]
pub fn score_breakdown(input: &ScoreInput<'_>) -> ReplayScoreBreakdown {
    let h_success = input.health.model_success_rate(
        &input.candidate.capability.provider,
        &input.candidate.credential_id,
        &input.candidate.capability.model.to_string(),
    );
    let quota_capacity = input.headroom;
    let q_cooldown_secs = input.cooldown_secs;
    let m_affinity = input.route_preference;
    let hash_bias = input.client_affinity;
    let l_band = ladder_band_index(
        &input.candidate.capability.provider,
        &input.candidate.credential_tier,
        &input.candidate.capability.model.to_string(),
    );
    let cost_class = cost_class_label(input.candidate.credential_cost_class);
    let cost = f64::from(input.candidate.credential_cost_class.rank_base());
    let score = W_HEALTH * h_success
        + W_HEADROOM * quota_capacity
        + W_ROUTE_PREFERENCE * m_affinity
        + W_CLIENT_AFFINITY * hash_bias
        + W_COST * (1.0 / (1.0 + cost))
        - W_COOLDOWN * norm_cooldown(q_cooldown_secs)
        - W_LADDER * f64::from(l_band);
    let (blocked_reason, next_available_at) = if quota_capacity <= 0.0 {
        (
            input
                .quota_blocked_reason
                .filter(|reason| *reason != BlockedReason::None),
            input.quota_next_available_at.clone(),
        )
    } else {
        (None, None)
    };
    ReplayScoreBreakdown {
        score,
        h_success,
        quota_capacity,
        q_cooldown_secs,
        m_affinity,
        hash_bias,
        l_band,
        cost_class,
        blocked_reason,
        next_available_at,
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

/// Pick a stable spread index among sorted feasible account ids.
#[must_use]
pub fn spread_pool_index(
    agent_name: &str,
    work_unit_id: &str,
    sorted_credentials: &[String],
) -> usize {
    if sorted_credentials.is_empty() {
        return 0;
    }
    if let Some(ordinal) = explicit_work_unit_ordinal(work_unit_id) {
        return ordinal % sorted_credentials.len();
    }
    spread_slot_index(
        agent_name,
        work_unit_id,
        "first-hop-spread",
        sorted_credentials.len(),
    )
}

fn explicit_work_unit_ordinal(work_unit_id: &str) -> Option<usize> {
    let suffix = work_unit_id.rsplit('-').next()?;
    let number = suffix.parse::<usize>().ok()?;
    Some(number.saturating_sub(1))
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

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::spread_pool_index;

    #[test]
    fn spread_pool_index_yields_eight_distinct_for_unit_suffixes() {
        let peers: Vec<String> = (1..=16)
            .map(|index| {
                if index == 1 {
                    "gemini-free".to_string()
                } else {
                    format!("gemini-free-{index}")
                }
            })
            .collect();
        let mut picks = HashSet::new();
        for unit in 1..=8 {
            let idx = spread_pool_index(
                &format!("admission-spread-{unit}"),
                &format!("unit-{unit}"),
                &peers,
            );
            picks.insert(peers[idx].clone());
        }
        assert!(
            picks.len() >= 8,
            "expected eight distinct spread picks, got {picks:?}"
        );
    }

    #[test]
    fn explicit_work_unit_ordinal_parses_unit_suffix() {
        assert_eq!(super::explicit_work_unit_ordinal("unit-8"), Some(7));
        assert!(super::explicit_work_unit_ordinal("opaque-id").is_none());
    }
}
