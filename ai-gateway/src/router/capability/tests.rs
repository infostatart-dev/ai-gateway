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

    let body = Bytes::from(r#"{"model": "openai/gpt-5-mini"}"#);
    let reqs = extract_requirements(&body);
    assert!(!reqs.reasoning_preferred);
    assert_eq!(
        reqs.preferred_intent_tier,
        Some(crate::router::intent::IntentTier::FastThinking)
    );

    let body = Bytes::from(r#"{"model": "openai/gpt-5"}"#);
    let reqs = extract_requirements(&body);
    assert!(reqs.reasoning_preferred);
    assert_eq!(
        reqs.preferred_intent_tier,
        Some(crate::router::intent::IntentTier::Deep)
    );
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
fn capability_fit_score_prefers_reasoning_and_json_schema_matches() {
    let reqs = RequestRequirements {
        json_schema_required: true,
        reasoning_preferred: true,
        ..RequestRequirements::default()
    };
    let full = ModelCapability {
        provider: InferenceProvider::OpenRouter,
        model: test_model(
            InferenceProvider::OpenRouter,
            "openai/gpt-oss-120b:free",
        ),
        context_window: Some(131_072),
        supports_tools: true,
        supports_json_schema: true,
        supports_vision: false,
        reasoning: true,
        json_schema_rank: 0,
        intent_tier: IntentTier::Standard,
    };
    let json_only = ModelCapability {
        reasoning: false,
        ..full.clone()
    };

    assert!(
        capability_fit_score(&reqs, &full)
            > capability_fit_score(&reqs, &json_only)
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
        json_schema_rank: 0,
        intent_tier: IntentTier::Standard,
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
    assert!(supports(&reqs, &model));
    assert!(!supports_with_payload(&reqs, &model, model.context_window));

    // Fail-open: unknown effective window must not filter the candidate.
    let model_unknown_context = ModelCapability {
        context_window: None,
        ..model.clone()
    };
    assert!(supports_with_payload(&reqs, &model_unknown_context, None));
}

#[test]
fn deepseek_web_supports_json_schema_for_autodefault() {
    let provider = InferenceProvider::Named("deepseek-web".into());
    let model = test_model(provider.clone(), "deepseek-chat");
    let cap = get_model_capability(&provider, &model, None);
    assert!(cap.supports_json_schema);

    let reqs = RequestRequirements {
        json_schema_required: true,
        ..RequestRequirements::default()
    };
    assert!(supports(&reqs, &cap));
}

#[test]
fn deepseek_web_excluded_when_tools_required() {
    let provider = InferenceProvider::Named("deepseek-web".into());
    let model = test_model(provider.clone(), "deepseek-chat");
    let cap = get_model_capability(&provider, &model, None);
    assert!(!cap.supports_tools);

    let reqs = RequestRequirements {
        tools_required: true,
        ..RequestRequirements::default()
    };
    assert!(!supports(&reqs, &cap));
}

#[test]
fn longcat_flash_lite_supports_json_schema() {
    use super::providers::apply_provider_capabilities;
    let provider = InferenceProvider::Named("longcat".into());
    let mut cap = ModelCapability {
        provider: provider.clone(),
        model: ModelId::from_str_and_provider(
            provider.clone(),
            "LongCat-Flash-Lite",
        )
        .unwrap(),
        context_window: None,
        supports_tools: false,
        supports_json_schema: false,
        supports_vision: false,
        reasoning: false,
        json_schema_rank: 0,
        intent_tier: IntentTier::Standard,
    };
    apply_provider_capabilities(&mut cap, &provider, "LongCat-Flash-Lite");
    assert!(cap.supports_json_schema);
}

#[test]
fn github_models_o1_is_reasoning_without_tools_or_json_schema() {
    use crate::config::providers::ProvidersConfig;

    let provider = InferenceProvider::Named("github-models".into());
    let model = test_model(provider.clone(), "openai/o1");
    let metadata = ProvidersConfig::default()
        .get(&provider)
        .and_then(|cfg| cfg.model_capabilities.get(&model))
        .cloned();
    let cap = get_model_capability(&provider, &model, metadata.as_ref());
    assert!(cap.reasoning);
    assert!(!cap.supports_tools);
    assert!(!cap.supports_json_schema);
}

#[test]
fn github_models_grok_3_excluded_for_json_schema_routing() {
    use crate::config::providers::ProvidersConfig;

    let provider = InferenceProvider::Named("github-models".into());
    let model = test_model(provider.clone(), "xai/grok-3");
    let metadata = ProvidersConfig::default()
        .get(&provider)
        .and_then(|cfg| cfg.model_capabilities.get(&model))
        .cloned();
    let cap = get_model_capability(&provider, &model, metadata.as_ref());
    assert!(!cap.supports_json_schema);

    let reqs = RequestRequirements {
        json_schema_required: true,
        ..RequestRequirements::default()
    };
    assert!(!supports(&reqs, &cap));
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

        // OpenAI catalog: gpt-4 supports vision — expect non-empty
        // vision-capable candidates.
        let candidates = router.ordered_candidates(&reqs, None, None).unwrap();
        assert!(!candidates.is_empty());
        assert!(candidates.iter().all(|c| c.capability.supports_vision));

        // Payload footprint filtering lives in budget-aware routers; capability
        // router matches capability flags only.
        reqs.min_context_tokens = Some(10_000_000);
        let result = router.ordered_candidates(&reqs, None, None);
        assert!(result.is_ok());
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
                None,
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
                    "gemini-2.5-flash".to_string(),
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
    let m = ModelId::from_str_and_provider(InferenceProvider::OpenAI, "gpt-4o")
        .unwrap();
    assert!(cfg.is_empty());
    assert_eq!(cfg.tier_of(&m), None);
}
