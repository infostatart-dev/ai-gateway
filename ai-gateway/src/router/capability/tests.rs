use super::*;
use crate::{
    config::decision::{DecisionTier, TierCascade},
    types::model_id::Version,
};

fn test_model(provider: InferenceProvider, name: &str) -> ModelId {
    ModelId::ModelIdWithVersion {
        provider,
        id: crate::types::model_id::ModelIdWithVersion {
            model: name.to_string(),
            version: Version::ImplicitLatest,
        },
    }
}

#[test]
fn test_extract_requirements_tools() {
    let body =
        Bytes::from(r#"{"model": "gpt-4", "tools": [{"type": "function"}]}"#);
    let reqs = extract_requirements(&body);
    assert!(reqs.tools_required);
    assert!(!reqs.json_schema_required);
    assert!(!reqs.vision_required);
}

#[test]
fn test_extract_requirements_json_schema() {
    let body = Bytes::from(
        r#"{"model": "gpt-4", "response_format": {"type": "json_schema"}}"#,
    );
    let reqs = extract_requirements(&body);
    assert!(!reqs.tools_required);
    assert!(reqs.json_schema_required);
}

#[test]
fn test_extract_requirements_vision() {
    let body = Bytes::from(
        r#"{
            "model": "gpt-4",
            "messages": [
                {
                    "role": "user",
                    "content": [
                        {"type": "text", "text": "What is in this image?"},
                        {"type": "image_url", "image_url": {"url": "..."}}
                    ]
                }
            ]
        }"#,
    );
    let reqs = extract_requirements(&body);
    assert!(reqs.vision_required);
}

#[test]
fn test_extract_requirements_reasoning() {
    let body = Bytes::from(r#"{"model": "o1-mini"}"#);
    let reqs = extract_requirements(&body);
    assert!(reqs.reasoning_preferred);

    let body = Bytes::from(r#"{"model": "gpt-4o"}"#);
    let reqs = extract_requirements(&body);
    assert!(!reqs.reasoning_preferred);
}

#[test]
fn test_extract_source_model_accepts_prefixed_and_plain_openai_models() {
    let prefixed = Bytes::from(r#"{"model": "openai/gpt-4o-mini"}"#);
    let plain = Bytes::from(r#"{"model": "gpt-4o-mini"}"#);

    assert_eq!(
        extract_source_model(&prefixed),
        extract_source_model(&plain)
    );
    assert_eq!(
        extract_source_model(&prefixed).unwrap().to_string(),
        "gpt-4o-mini"
    );
}

#[test]
fn test_supports_logic() {
    let model = ModelCapability {
        provider: InferenceProvider::OpenAI,
        model: test_model(InferenceProvider::OpenAI, "gpt-4"),
        context_window: Some(128_000),
        supports_tools: true,
        supports_json_schema: true,
        supports_vision: true,
        reasoning: false,
    };

    let mut reqs = RequestRequirements::default();
    assert!(supports(&reqs, &model));

    reqs.tools_required = true;
    assert!(supports(&reqs, &model));

    let model_no_tools = ModelCapability {
        supports_tools: false,
        ..model.clone()
    };
    assert!(!supports(&reqs, &model_no_tools));

    reqs.tools_required = false;
    reqs.min_context_tokens = Some(200_000);
    assert!(!supports(&reqs, &model));

    // Strict context check: None context should NOT pass if min_context is
    // specified
    let model_unknown_context = ModelCapability {
        context_window: None,
        ..model.clone()
    };
    assert!(!supports(&reqs, &model_unknown_context));
}

#[cfg(feature = "testing")]
mod async_tests {
    use std::sync::Arc;

    use super::*;
    use crate::{config::router::RouterConfig, types::router::RouterId};

    #[tokio::test]
    async fn test_ordered_candidates_hard_requirements() {
        let app_state = AppState::test_default().await;
        let router_id = RouterId::Named("test".into());
        let router_config = Arc::new(RouterConfig::default());
        let providers = nonempty_collections::nes![InferenceProvider::OpenAI];

        let router = CapabilityAwareRouter::new(
            app_state,
            router_id,
            router_config,
            &providers,
        )
        .await
        .unwrap();

        let mut reqs = RequestRequirements {
            vision_required: true,
            ..RequestRequirements::default()
        };

        // OpenAI catalog: gpt-4 supports vision — expect non-empty vision-capable candidates.
        let candidates = router.ordered_candidates(&reqs, None).unwrap();
        assert!(!candidates.is_empty());
        assert!(candidates.iter().all(|c| c.capability.supports_vision));

        // Impossible requirement: huge context window.
        reqs.min_context_tokens = Some(10_000_000);
        let result = router.ordered_candidates(&reqs, None);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_ordered_candidates_are_limited_to_mapped_models() {
        let app_state = AppState::test_default().await;
        let router_id = RouterId::Named("test".into());
        let router_config = Arc::new(RouterConfig::default());
        let providers = nonempty_collections::nes![
            InferenceProvider::Anthropic,
            InferenceProvider::GoogleGemini,
            InferenceProvider::OpenRouter,
            InferenceProvider::Named("groq".into())
        ];

        let router = CapabilityAwareRouter::new(
            app_state,
            router_id,
            router_config,
            &providers,
        )
        .await
        .unwrap();

        let source_model = ModelId::from_str("openai/gpt-4o-mini").unwrap();
        let candidates = router
            .ordered_candidates(
                &RequestRequirements::default(),
                Some(&source_model),
            )
            .unwrap();

        let mut candidate_models: Vec<_> = candidates
            .iter()
            .map(|c| {
                (
                    c.capability.provider.clone(),
                    c.capability.model.to_string(),
                )
            })
            .collect();
        candidate_models.sort_by(|a, b| {
            a.0.to_string()
                .cmp(&b.0.to_string())
                .then_with(|| a.1.cmp(&b.1))
        });

        assert_eq!(
            candidate_models,
            vec![
                (InferenceProvider::Anthropic, "claude-3-5-haiku".to_string(),),
                (
                    InferenceProvider::GoogleGemini,
                    "gemini-2.0-flash".to_string(),
                ),
                (
                    InferenceProvider::Named("groq".into()),
                    "llama-3.1-8b-instant".to_string(),
                ),
                (
                    InferenceProvider::OpenRouter,
                    "openai/gpt-4o-mini".to_string(),
                ),
            ]
        );
    }

}

// ─── tier-cascade chain helpers ─────────────────────────────────────────

#[test]
fn tier_chain_only_tier_returns_single_start() {
    assert_eq!(
        tier_chain_for_models(Tier::Paid, TierCascade::OnlyTier),
        vec![Tier::Paid]
    );
    assert_eq!(
        tier_chain_for_models(Tier::Freemium, TierCascade::OnlyTier),
        vec![Tier::Freemium]
    );
}

#[test]
fn tier_chain_paid_down_starts_from_given() {
    assert_eq!(
        tier_chain_for_models(Tier::Paid, TierCascade::PaidDown),
        vec![Tier::Paid, Tier::Freemium, Tier::Free]
    );
    assert_eq!(
        tier_chain_for_models(Tier::Freemium, TierCascade::PaidDown),
        vec![Tier::Freemium, Tier::Free]
    );
    assert_eq!(
        tier_chain_for_models(Tier::Free, TierCascade::PaidDown),
        vec![Tier::Free]
    );
}

#[test]
fn tier_chain_free_up_starts_from_given() {
    assert_eq!(
        tier_chain_for_models(Tier::Free, TierCascade::FreeUp),
        vec![Tier::Free, Tier::Freemium, Tier::Paid]
    );
    assert_eq!(
        tier_chain_for_models(Tier::Freemium, TierCascade::FreeUp),
        vec![Tier::Freemium, Tier::Paid]
    );
    assert_eq!(
        tier_chain_for_models(Tier::Paid, TierCascade::FreeUp),
        vec![Tier::Paid]
    );
}

// ─── ModelTiersConfig::tier_of ──────────────────────────────────────────

#[test]
fn model_tiers_resolves_full_qualified_id() {
    let yaml = r#"
free:
  - "openai/gpt-oss-20b:free"
freemium:
  - "openai/gpt-4o-mini"
paid:
  - "openai/gpt-4o"
"#;
    let cfg: crate::config::decision::ModelTiersConfig =
        serde_yml::from_str(yaml).unwrap();

    let m_free = ModelId::from_str_and_provider(
        InferenceProvider::OpenRouter,
        "openai/gpt-oss-20b:free",
    )
    .unwrap();
    let m_freemium = ModelId::from_str_and_provider(
        InferenceProvider::OpenAI,
        "gpt-4o-mini",
    )
    .unwrap();
    let m_paid =
        ModelId::from_str_and_provider(InferenceProvider::OpenAI, "gpt-4o")
            .unwrap();
    let m_unknown = ModelId::from_str_and_provider(
        InferenceProvider::OpenAI,
        "gpt-3.5-turbo",
    )
    .unwrap();

    assert_eq!(cfg.tier_of(&m_free), Some(DecisionTier::Free));
    assert_eq!(cfg.tier_of(&m_freemium), Some(DecisionTier::Freemium));
    assert_eq!(cfg.tier_of(&m_paid), Some(DecisionTier::Paid));
    assert_eq!(cfg.tier_of(&m_unknown), None);
}

#[test]
fn model_tiers_empty_returns_none() {
    let cfg = crate::config::decision::ModelTiersConfig::default();
    let m = ModelId::from_str_and_provider(
        InferenceProvider::OpenAI,
        "gpt-4o",
    )
    .unwrap();
    assert!(cfg.is_empty());
    assert_eq!(cfg.tier_of(&m), None);
}
