//! Test-only router builders for routing load verification.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Duration,
};

use indexmap::IndexMap;

use super::{
    credential_balance::CredentialRoundRobin,
    types::{BudgetAwareRouter, BudgetCandidate, CandidateSelectionMode},
};
use crate::{
    app_state::AppState,
    config::{
        cost_class::CostClass, credentials::ProviderCredentialId,
        router::RouterConfig,
    },
    dispatcher::Dispatcher,
    endpoints::EndpointType,
    middleware::mapper::model::ModelMapper,
    router::capability::ModelCapability,
    types::{
        model_id::ModelId,
        provider::{InferenceProvider, ProviderKey},
        router::RouterId,
        secret::Secret,
    },
};

pub(crate) fn intent_autodefault_router(
    app_state: &AppState,
    candidates: Vec<BudgetCandidate>,
) -> BudgetAwareRouter {
    let router_config = Arc::new(RouterConfig {
        source_model_selection: Some(
            crate::config::router::SourceModelSelection::Intent,
        ),
        ..Default::default()
    });
    BudgetAwareRouter {
        app_state: app_state.clone(),
        router_id: RouterId::Named("autodefault".into()),
        endpoint_type: EndpointType::Chat,
        strategy: "budget-aware-capability-after",
        candidates: Arc::new(candidates),
        model_mapper: ModelMapper::new_for_router(
            app_state.clone(),
            router_config.clone(),
        ),
        states: Arc::new(Mutex::new(HashMap::new())),
        model_states: Arc::new(Mutex::new(HashMap::new())),
        provider_priorities: Arc::new(IndexMap::new()),
        default_latency: Duration::from_millis(10),
        max_cooldown_wait: Duration::from_secs(0),
        selection_mode: CandidateSelectionMode::BudgetThenCapability,
        credential_round_robin: CredentialRoundRobin::new_shared(),
        source_model_selection:
            crate::config::router::SourceModelSelection::Intent,
    }
}

pub(crate) async fn scout_candidate(
    app_state: &AppState,
    credential_id: &str,
) -> BudgetCandidate {
    let provider = InferenceProvider::Named("groq".into());
    let cred = ProviderCredentialId::new(credential_id);
    let model_id = ModelId::from_str_and_provider(
        provider.clone(),
        "meta-llama/llama-4-scout-17b-16e-instruct",
    )
    .expect("model");
    let router_config = Arc::new(RouterConfig::default());
    let service = Dispatcher::new_with_model_id_and_provider_key_without_rate_limit_events(
        app_state.clone(),
        &RouterId::Named("routing-load".into()),
        &router_config,
        provider.clone(),
        model_id.clone(),
        Some(&ProviderKey::Secret(Secret::from("groq-key".to_string()))),
        Some(&cred),
    )
    .await
    .expect("dispatcher");
    BudgetCandidate {
        credential_id: cred,
        credential_budget_rank: 0,
        credential_cost_class: CostClass::Free,
        credential_tier: "free".into(),
        capability: ModelCapability {
            provider,
            model: model_id,
            context_window: Some(131_072),
            supports_tools: true,
            supports_json_schema: true,
            supports_vision: false,
            reasoning: false,
            json_schema_rank: 1,
            intent_tier: crate::router::intent::IntentTier::FastThinking,
        },
        service,
    }
}

pub(crate) async fn deep_paid_candidate(
    app_state: &AppState,
) -> BudgetCandidate {
    let provider = InferenceProvider::Anthropic;
    let cred = ProviderCredentialId::new("anthropic-test");
    let model_id =
        ModelId::from_str_and_provider(provider.clone(), "claude-sonnet-4-0")
            .expect("model");
    let router_config = Arc::new(RouterConfig::default());
    let service = Dispatcher::new_with_model_id_and_provider_key_without_rate_limit_events(
        app_state.clone(),
        &RouterId::Named("routing-load".into()),
        &router_config,
        provider.clone(),
        model_id.clone(),
        Some(&ProviderKey::Secret(Secret::from("anthropic-key".to_string()))),
        Some(&cred),
    )
    .await
    .expect("dispatcher");
    BudgetCandidate {
        credential_id: cred,
        credential_budget_rank: 0,
        credential_cost_class: CostClass::Paid,
        credential_tier: "tier-3".into(),
        capability: ModelCapability {
            provider,
            model: model_id,
            context_window: Some(200_000),
            supports_tools: true,
            supports_json_schema: true,
            supports_vision: true,
            reasoning: true,
            json_schema_rank: 2,
            intent_tier: crate::router::intent::IntentTier::Deep,
        },
        service,
    }
}

pub(crate) fn empty_router(app_state: &AppState) -> BudgetAwareRouter {
    BudgetAwareRouter {
        app_state: app_state.clone(),
        router_id: RouterId::Named("routing-load".into()),
        endpoint_type: EndpointType::Chat,
        strategy: "budget-aware-capability-after",
        candidates: Arc::new(vec![]),
        model_mapper: ModelMapper::new_for_router(
            app_state.clone(),
            Arc::new(RouterConfig::default()),
        ),
        states: Arc::new(Mutex::new(HashMap::new())),
        model_states: Arc::new(Mutex::new(HashMap::new())),
        provider_priorities: Arc::new(IndexMap::new()),
        default_latency: Duration::from_millis(10),
        max_cooldown_wait: Duration::from_secs(0),
        selection_mode: CandidateSelectionMode::BudgetThenCapability,
        credential_round_robin: CredentialRoundRobin::new_shared(),
        source_model_selection:
            crate::config::router::SourceModelSelection::Strict,
    }
}

pub(crate) fn balance_ranked(
    router: &BudgetAwareRouter,
    ranked: Vec<BudgetCandidate>,
) -> Vec<BudgetCandidate> {
    router.credential_round_robin.balance(ranked)
}

pub(crate) fn router_with_candidates(
    app_state: &AppState,
    candidates: Vec<BudgetCandidate>,
) -> BudgetAwareRouter {
    let mut router = empty_router(app_state);
    router.candidates = Arc::new(candidates);
    router
}

pub(crate) fn ordered_candidates(
    router: &BudgetAwareRouter,
    requirements: &crate::router::capability::RequestRequirements,
) -> Result<Vec<BudgetCandidate>, crate::error::internal::InternalError> {
    router.ordered_candidates(requirements, None)
}

pub(crate) fn ordered_candidates_for_source(
    router: &BudgetAwareRouter,
    requirements: &crate::router::capability::RequestRequirements,
    source_model: &ModelId,
) -> Result<Vec<BudgetCandidate>, crate::error::internal::InternalError> {
    router.ordered_candidates(requirements, Some(source_model))
}

pub(crate) async fn gemini_model_candidate(
    app_state: &AppState,
    credential_id: &str,
    model: &str,
) -> BudgetCandidate {
    build_candidate(
        app_state,
        InferenceProvider::GoogleGemini,
        credential_id,
        0,
        "free-ladder-key",
        model,
        1_000_000,
    )
    .await
}

pub(crate) async fn openrouter_model_candidate(
    app_state: &AppState,
    credential_id: &str,
    model: &str,
) -> BudgetCandidate {
    build_candidate(
        app_state,
        InferenceProvider::OpenRouter,
        credential_id,
        0,
        "openrouter-key",
        model,
        131_072,
    )
    .await
}

pub(crate) async fn gemini_candidate(
    app_state: &AppState,
    credential_id: &str,
    budget_rank: u16,
    key: &str,
) -> BudgetCandidate {
    build_candidate(
        app_state,
        InferenceProvider::GoogleGemini,
        credential_id,
        budget_rank,
        key,
        "gemini-2.5-flash",
        1_000_000,
    )
    .await
}

pub(crate) async fn groq_candidate(app_state: &AppState) -> BudgetCandidate {
    build_candidate(
        app_state,
        InferenceProvider::Named("groq".into()),
        "groq-default",
        0,
        "groq-key",
        "llama-3.3-70b-versatile",
        131_072,
    )
    .await
}

pub(crate) async fn chatgpt_candidate(app_state: &AppState) -> BudgetCandidate {
    let provider = InferenceProvider::Named("chatgpt-web".into());
    let cred = ProviderCredentialId::new("chatgpt-web-default");
    let model_id =
        ModelId::from_str_and_provider(provider.clone(), "gpt-5.4").unwrap();
    let router_config = Arc::new(RouterConfig::default());
    let service = Dispatcher::new_with_model_id_and_provider_key_without_rate_limit_events(
        app_state.clone(),
        &RouterId::Named("routing-load".into()),
        &router_config,
        provider.clone(),
        model_id.clone(),
        None,
        Some(&cred),
    )
    .await
    .expect("dispatcher");
    BudgetCandidate {
        credential_id: cred,
        credential_budget_rank: 0,
        credential_cost_class: CostClass::PaidBrowser,
        credential_tier: "plus-single-session".into(),
        capability: ModelCapability {
            provider,
            model: model_id,
            context_window: Some(128_000),
            supports_tools: true,
            supports_json_schema: true,
            supports_vision: true,
            reasoning: false,
            json_schema_rank: 1,
            intent_tier: crate::router::intent::IntentTier::Standard,
        },
        service,
    }
}

pub(crate) async fn gemini_slots(
    app_state: &AppState,
    count: u8,
) -> Vec<BudgetCandidate> {
    let mut out = Vec::new();
    for index in 1..=count {
        let id = if index == 1 {
            "gemini-free".to_string()
        } else {
            format!("gemini-free-{index}")
        };
        out.push(
            gemini_candidate(app_state, &id, 0, &format!("free-{index}-key"))
                .await,
        );
    }
    out
}

#[must_use]
pub(crate) fn request_parts() -> http::request::Parts {
    http::Request::builder()
        .method(http::Method::POST)
        .uri("/v1/chat/completions")
        .body(())
        .unwrap()
        .into_parts()
        .0
}

async fn build_candidate(
    app_state: &AppState,
    provider: InferenceProvider,
    credential_id: &str,
    budget_rank: u16,
    key: &str,
    model: &str,
    context_window: u32,
) -> BudgetCandidate {
    let cred = ProviderCredentialId::new(credential_id);
    let model_id =
        ModelId::from_str_and_provider(provider.clone(), model).unwrap();
    let router_config = Arc::new(RouterConfig::default());
    let service = Dispatcher::new_with_model_id_and_provider_key_without_rate_limit_events(
        app_state.clone(),
        &RouterId::Named("routing-load".into()),
        &router_config,
        provider.clone(),
        model_id.clone(),
        Some(&ProviderKey::Secret(Secret::from(key.to_string()))),
        Some(&cred),
    )
    .await
    .expect("dispatcher");
    BudgetCandidate {
        credential_id: cred,
        credential_budget_rank: budget_rank,
        credential_cost_class: if budget_rank == 0 {
            CostClass::Free
        } else {
            CostClass::Paid
        },
        credential_tier: if budget_rank == 0 {
            "free".into()
        } else {
            "tier-3".into()
        },
        capability: ModelCapability {
            provider,
            model: model_id,
            context_window: Some(context_window),
            supports_tools: true,
            supports_json_schema: true,
            supports_vision: true,
            reasoning: false,
            json_schema_rank: 2,
            intent_tier: crate::router::intent::IntentTier::Standard,
        },
        service,
    }
}
