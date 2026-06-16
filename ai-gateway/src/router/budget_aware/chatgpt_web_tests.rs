//! chatgpt-web routing: no cross-provider model yaml matching.

use std::{path::PathBuf, sync::Arc, time::Duration};

use indexmap::IndexMap;
use nonempty_collections::nes;

use super::{
    BudgetAwareRouter,
    factory::build,
    types::{BudgetCandidate, CandidateSelectionMode},
};
use crate::{
    app_state::AppState,
    config::{
        chatgpt_web,
        router::RouterConfig,
        secrets_file::SECRETS_FILE_ENV,
    },
    endpoints::EndpointType,
    router::capability::RequestRequirements,
    types::{model_id::ModelId, provider::InferenceProvider, router::RouterId},
};

fn ensure_chatgpt_session_secrets() {
    let session =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../dev/session.json");
    if !session.exists() {
        return;
    }
    let dir = std::env::temp_dir()
        .join(format!("ai-gw-chatgpt-test-secrets-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let secrets_path = dir.join("secrets.yaml");
    std::fs::write(
        &secrets_path,
        format!(
            "credentials:\n  {}:\n    session-file: {}\n",
            chatgpt_web::DEFAULT_CREDENTIAL_ID,
            session.display()
        ),
    )
    .unwrap();
    unsafe {
        std::env::set_var(SECRETS_FILE_ENV, &secrets_path);
    }
}

fn client_model(name: &str) -> ModelId {
    ModelId::from_str_and_provider(
        InferenceProvider::Named("chatgpt-web".into()),
        name,
    )
    .expect("model id")
}

async fn chatgpt_only_router() -> BudgetAwareRouter {
    ensure_chatgpt_session_secrets();
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
    let candidate: &BudgetCandidate =
        router.candidates.first().expect("chatgpt-web candidate");
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
