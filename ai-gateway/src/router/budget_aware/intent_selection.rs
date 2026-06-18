//! Intent-mode candidate filtering and preferred/escalation band ordering.

use super::types::{BudgetAwareRouter, BudgetCandidate};
use crate::{
    config::{chatgpt_web::is_chatgpt_web, deepseek_web::is_deepseek_web},
    router::{
        capability::RequestRequirements,
        intent::{IntentTier, RoutingIntent, extract_routing_intent},
    },
    types::model_id::ModelId,
};

pub(super) fn routing_intent_for_request(
    source_model: Option<&ModelId>,
) -> RoutingIntent {
    source_model.map(extract_routing_intent).unwrap_or_default()
}

pub(super) fn candidate_matches_intent(
    candidate: &BudgetCandidate,
    intent: RoutingIntent,
    requirements: &RequestRequirements,
) -> bool {
    if is_chatgpt_web(&candidate.capability.provider)
        || is_deepseek_web(&candidate.capability.provider)
    {
        return true;
    }
    candidate.capability.intent_tier >= intent.effective_floor(requirements)
}

pub(super) fn passes_source_selection(
    router: &BudgetAwareRouter,
    source_model: Option<&ModelId>,
    candidate: &BudgetCandidate,
    requirements: &RequestRequirements,
    intent: RoutingIntent,
) -> bool {
    if router.source_model_selection
        == crate::config::router::SourceModelSelection::Intent
    {
        return candidate_matches_intent(candidate, intent, requirements);
    }
    source_model.is_none_or(|model| {
        router.matches_source_model(model, candidate, requirements)
    })
}

pub(super) fn order_intent_bands(
    router: &BudgetAwareRouter,
    mut candidates: Vec<BudgetCandidate>,
    requirements: &RequestRequirements,
    intent: RoutingIntent,
) -> Vec<BudgetCandidate> {
    let mut preferred = Vec::new();
    let mut escalation = Vec::new();

    for candidate in candidates.drain(..) {
        if in_preferred_band(&candidate, intent, requirements) {
            preferred.push(candidate);
        } else if candidate.capability.intent_tier > intent.preferred_tier
            && candidate.capability.intent_tier <= intent.escalation_ceiling
        {
            escalation.push(candidate);
        }
    }

    router.rank_candidates(&mut preferred, requirements, Some(intent));
    escalation.sort_by_key(|c| c.capability.intent_tier);
    router.rank_candidates(&mut escalation, requirements, Some(intent));

    preferred.append(&mut escalation);
    preferred
}

fn in_preferred_band(
    candidate: &BudgetCandidate,
    intent: RoutingIntent,
    requirements: &RequestRequirements,
) -> bool {
    if candidate.capability.intent_tier == intent.preferred_tier {
        return true;
    }
    !requirements.json_schema_required
        && intent.preferred_tier == IntentTier::FastThinking
        && candidate.capability.intent_tier == IntentTier::Fast
}

pub(super) fn selection_phase_for(
    intent: RoutingIntent,
    candidate: &BudgetCandidate,
) -> crate::router::intent::SelectionPhase {
    if candidate.capability.intent_tier == intent.preferred_tier {
        crate::router::intent::SelectionPhase::Preferred
    } else {
        crate::router::intent::SelectionPhase::Escalated
    }
}
