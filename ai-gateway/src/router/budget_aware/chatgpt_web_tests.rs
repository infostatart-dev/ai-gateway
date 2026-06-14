//! chatgpt-web routing: no cross-provider model yaml matching.

use std::{sync::Arc, time::Duration};

use indexmap::IndexMap;
use nonempty_collections::nes;

use super::{
    factory::build,
    types::{BudgetCandidate, CandidateSelectionMode},
    BudgetAwareRouter,
};
use crate::{
    app_state::AppState,
    config::router::RouterConfig,
    endpoints::EndpointType,
    router::capability::RequestRequirements,
    types::{model_id::ModelId, provider::InferenceProvider, router::RouterId},
};

fn client_model(name: &str) -> ModelId {
    ModelId::from_str_and_provider(
        InferenceProvider::Named("chatgpt-web".into()),
        name,
    )
    .expect("model id")
}

async fn chatgpt_only_router() -> BudgetAwareRouter {
    let app_state = AppState::test_default().await;
    let provider = InferenceProvider::Named("chatgpt-web".into());
    build(
        app_state,
        RouterId::Named("chatgpt-web-test".into()),
        Arc::new(RouterConfig::default()),
        &nes![provider],
        &IndexMap::new(),
        Duration::from_secs(3),
        CandidateSelectionMode::BudgetThenCapability,
        EndpointType::Chat,
        "test",
    )
    .await
    .expect("router")
}

#[tokio::test]
async fn matches_any_client_model_without_yaml_mapping() {
    let router = chatgpt_only_router().await;
    let source = client_model("gpt-5.5-instant");
    let candidate: &BudgetCandidate = router
        .candidates
        .first()
        .expect("chatgpt-web candidate");
    let requirements = RequestRequirements {
        json_schema_required: true,
        ..RequestRequirements::default()
    };

    assert!(router.matches_source_model(&source, candidate, &requirements));
}

#[tokio::test]
async fn ordered_candidates_include_chatgpt_for_unmapped_model() {
    let router = chatgpt_only_router().await;
    let source = client_model("gpt-5.5-instant");
    let requirements = RequestRequirements {
        json_schema_required: true,
        ..RequestRequirements::default()
    };

    let candidates = router
        .ordered_candidates(&requirements, Some(&source))
        .expect("candidates");

    assert_eq!(candidates.len(), 1);
    assert_eq!(
        candidates[0].capability.provider,
        InferenceProvider::Named("chatgpt-web".into())
    );
}
