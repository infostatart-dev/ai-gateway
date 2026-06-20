use std::{
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use ai_gateway::{
    app_state::AppState,
    config::{
        credentials::ProviderCredentialId,
        deepseek_web::session_path_for_credential,
        secrets_file::{InstalledSecretsGuard, SecretsFile},
    },
    tests::routing::{
        RequestRequirements, clear_test_call_responses,
        deepseek_model_candidate, empty_router, install_upstream_mock,
    },
};
use gateway_tests::{UpstreamMockScript, upstream::ok_chat_completion};

use crate::rl::support::*;

const MODEL: &str = "deepseek-chat";
const BLOCKED: &str = "deepseek-web-default";
const SIBLING: &str = "deepseek-web-2";

struct SessionFixture {
    _guard: InstalledSecretsGuard,
    _dir: PathBuf,
}

fn write_session(path: &Path) {
    std::fs::write(path, r#"{"token":"user-session-token"}"#).expect("session");
}

fn install_distinct_deepseek_sessions() -> SessionFixture {
    let dir = std::env::temp_dir()
        .join(format!("ai-gw-ds-admit-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("session dir");
    let first = dir.join("deepseek-a.json");
    let second = dir.join("deepseek-b.json");
    write_session(&first);
    write_session(&second);
    let secrets_path = dir.join("secrets.yaml");
    std::fs::write(&secrets_path, "credentials: {}\nintegrations: {}\n")
        .expect("secrets yaml");
    let mut secrets = SecretsFile::load(&secrets_path).expect("load secrets");
    secrets.register_session_path(BLOCKED, first);
    secrets.register_session_path(SIBLING, second);
    let guard = SecretsFile::install_for_test(secrets);
    SessionFixture {
        _guard: guard,
        _dir: dir,
    }
}

async fn block_default_session(app_state: &AppState) {
    let default_path =
        session_path_for_credential(BLOCKED).expect("default session");
    let sibling_path =
        session_path_for_credential(SIBLING).expect("sibling session");
    assert_ne!(
        default_path, sibling_path,
        "sessions must not share pacing scope"
    );

    let provider = ai_gateway::types::provider::InferenceProvider::Named(
        "deepseek-web".into(),
    );
    let gate = app_state
        .upstream_pacing()
        .gate_for(
            &provider,
            Some(&ProviderCredentialId::new(BLOCKED)),
            Some("free"),
            Some(MODEL),
        )
        .expect("default session gate");
    gate.apply_upstream_reconcile(Instant::now() + Duration::from_secs(120))
        .await;
}

pub async fn run() {
    clear_test_call_responses();
    install_upstream_mock(
        UpstreamMockScript::new()
            .binding(BLOCKED, MODEL, vec![ok_chat_completion])
            .binding(SIBLING, MODEL, vec![ok_chat_completion])
            .default_response(ok_chat_completion),
    );

    let app_state = AppState::test_default().await;
    let _sessions = install_distinct_deepseek_sessions();
    block_default_session(&app_state).await;

    let router = empty_router(&app_state);
    let pool = vec![
        deepseek_model_candidate(&app_state, BLOCKED, MODEL).await,
        deepseek_model_candidate(&app_state, SIBLING, MODEL).await,
    ];

    let result = run_planned_failover(
        router,
        caller_parts("deepseek-session", Some("unit-1")),
        default_fat_body(),
        pool,
        RequestRequirements::default(),
        None,
    )
    .await
    .expect("sibling session route");

    assert_eq!(
        routed_identity(&result.response),
        format!("{SIBLING}/{MODEL}")
    );
    assert_eq!(
        credential_attempts(&app_state, BLOCKED),
        0,
        "infeasible session must skip upstream HTTP"
    );
    assert_eq!(
        app_state
            .provider_stats_snapshot(None, None)
            .routing
            .repeat_429_violations,
        0
    );
}
