//! Payload-aware candidate filtering (effective window from catalog at rank
//! time).

use super::types::BudgetCandidate;
use crate::{
    config::provider_limits::ProviderLimitCatalog,
    router::{
        capability::{RequestRequirements, supports_with_payload},
        token_estimate::PayloadBudgetConfig,
    },
};

#[must_use]
pub fn candidate_effective_window(
    candidate: &BudgetCandidate,
    limits: &ProviderLimitCatalog,
    budget: PayloadBudgetConfig,
) -> Option<u32> {
    let token_cap = limits.per_request_token_cap(
        &candidate.capability.provider,
        &candidate.credential_tier,
        &candidate.capability.model.to_string(),
    );
    crate::router::capability::effective_routing_window(
        candidate.capability.context_window,
        token_cap,
        budget,
    )
}

/// Filter ranked candidates by capability + payload; best-effort tail when all
/// are oversized (D2).
pub fn filter_payload_capable(
    ranked: Vec<BudgetCandidate>,
    requirements: &RequestRequirements,
    limits: &ProviderLimitCatalog,
    budget: PayloadBudgetConfig,
    source_matches: impl Fn(&BudgetCandidate) -> bool,
) -> Vec<BudgetCandidate> {
    let fits = |c: &BudgetCandidate| {
        source_matches(c)
            && supports_with_payload(
                requirements,
                &c.capability,
                candidate_effective_window(c, limits, budget),
            )
    };

    let supported: Vec<_> =
        ranked.iter().filter(|c| fits(c)).cloned().collect();
    if !supported.is_empty() || requirements.min_context_tokens.is_none() {
        return supported;
    }

    let relaxed = RequestRequirements {
        min_context_tokens: None,
        ..requirements.clone()
    };
    let best_effort: Vec<_> = ranked
        .into_iter()
        .filter(|c| {
            source_matches(c)
                && supports_with_payload(
                    &relaxed,
                    &c.capability,
                    candidate_effective_window(c, limits, budget),
                )
        })
        .collect();
    keep_largest_effective_window(best_effort, limits, budget)
}

fn keep_largest_effective_window(
    mut candidates: Vec<BudgetCandidate>,
    limits: &ProviderLimitCatalog,
    budget: PayloadBudgetConfig,
) -> Vec<BudgetCandidate> {
    let Some(max) = candidates
        .iter()
        .filter_map(|c| candidate_effective_window(c, limits, budget))
        .max()
    else {
        return candidates;
    };
    candidates
        .retain(|c| candidate_effective_window(c, limits, budget) == Some(max));
    candidates
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        config::provider_limits::ProviderLimitCatalog,
        router::{
            capability::{ModelCapability, effective_routing_window},
            token_estimate::PayloadBudgetConfig,
        },
        types::provider::InferenceProvider,
    };

    fn budget() -> PayloadBudgetConfig {
        PayloadBudgetConfig::default()
    }

    fn limits() -> ProviderLimitCatalog {
        ProviderLimitCatalog::default()
    }

    #[test]
    fn groq_free_tpm_caps_effective_window() {
        let groq = InferenceProvider::Named("groq".into());
        let tpm = limits().per_request_token_cap(
            &groq,
            "free",
            "llama-3.3-70b-versatile",
        );
        assert_eq!(tpm, Some(12_000));
        let window = effective_routing_window(Some(131_072), tpm, budget());
        assert_eq!(window, Some(11_400));
    }

    #[test]
    fn openrouter_context_margin_filters_128k_plus_output() {
        let window = effective_routing_window(Some(131_072), None, budget());
        assert_eq!(window, Some(124_518));
        let reqs = RequestRequirements {
            min_context_tokens: Some(132_000),
            ..Default::default()
        };
        let model = ModelCapability {
            provider: InferenceProvider::OpenRouter,
            model: crate::types::model_id::ModelId::from_str_and_provider(
                InferenceProvider::OpenRouter,
                "openai/gpt-4o-mini",
            )
            .unwrap(),
            context_window: Some(131_072),
            supports_tools: true,
            supports_json_schema: true,
            supports_vision: false,
            reasoning: false,
            json_schema_rank: -1,
        };
        assert!(!supports_with_payload(&reqs, &model, window));
    }

    #[test]
    fn unknown_window_fail_open() {
        let reqs = RequestRequirements {
            min_context_tokens: Some(999_999),
            ..Default::default()
        };
        let model = ModelCapability {
            provider: InferenceProvider::Named("unknown".into()),
            model: crate::types::model_id::ModelId::from_str_and_provider(
                InferenceProvider::Named("unknown".into()),
                "m",
            )
            .unwrap(),
            context_window: None,
            supports_tools: true,
            supports_json_schema: true,
            supports_vision: false,
            reasoning: false,
            json_schema_rank: 0,
        };
        assert!(supports_with_payload(&reqs, &model, None));
    }
}
