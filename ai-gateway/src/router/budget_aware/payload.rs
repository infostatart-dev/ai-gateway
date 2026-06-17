//! Payload-aware candidate filtering (effective window from catalog at rank
//! time).

use super::types::BudgetCandidate;
use crate::{
    config::{
        chatgpt_web::is_chatgpt_web, deepseek_web::is_deepseek_web,
        provider_limits::ProviderLimitCatalog,
    },
    router::{
        capability::{RequestRequirements, supports_with_payload},
        token_estimate::PayloadBudgetConfig,
    },
    types::provider::InferenceProvider,
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

fn is_web_session_provider(provider: &InferenceProvider) -> bool {
    is_chatgpt_web(provider) || is_deepseek_web(provider)
}

/// Filter ranked candidates by capability + payload. No best-effort fallback
/// when every API-key provider is oversized.
pub fn filter_payload_capable(
    ranked: Vec<BudgetCandidate>,
    requirements: &RequestRequirements,
    limits: &ProviderLimitCatalog,
    budget: PayloadBudgetConfig,
    source_matches: impl Fn(&BudgetCandidate) -> bool,
) -> Vec<BudgetCandidate> {
    let fits = |c: &BudgetCandidate| {
        if !source_matches(c) {
            return false;
        }
        if is_web_session_provider(&c.capability.provider) {
            return crate::router::capability::capability_supports(
                requirements,
                &c.capability,
            );
        }
        supports_with_payload(
            requirements,
            &c.capability,
            candidate_effective_window(c, limits, budget),
        )
    };

    ranked.into_iter().filter(fits).collect()
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

    #[test]
    fn oversized_payload_excludes_openrouter_window() {
        let reqs = RequestRequirements {
            min_context_tokens: Some(158_000),
            ..Default::default()
        };
        let window = effective_routing_window(Some(131_072), None, budget());
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
}
