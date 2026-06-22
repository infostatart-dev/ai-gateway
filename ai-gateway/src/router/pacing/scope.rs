use crate::{
    config::{
        chatgpt_web::{
            is_chatgpt_web, session_path_for_credential as chatgpt_session_path,
        },
        credentials::ProviderCredentialId,
        deepseek_web::{
            is_deepseek_web,
            session_path_for_credential as deepseek_session_path,
        },
        perplexity_web::{
            is_perplexity_web,
            session_path_for_credential as perplexity_session_path,
        },
        provider_limits::ProviderQuotaProfile,
    },
    types::provider::InferenceProvider,
};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum PacingScope {
    Session(String),
    Credential(String),
    CredentialModel { credential: String, model: String },
}

#[must_use]
pub fn resolve_pacing_scope(
    provider: &InferenceProvider,
    credential_id: Option<&ProviderCredentialId>,
    model: Option<&str>,
    quota_profile: ProviderQuotaProfile,
) -> PacingScope {
    if is_chatgpt_web(provider) {
        let path = session_scope_key(
            credential_id,
            credential_id
                .map(ProviderCredentialId::as_str)
                .and_then(chatgpt_session_path),
        );
        return PacingScope::Session(path);
    }
    if is_deepseek_web(provider) {
        let path = session_scope_key(
            credential_id,
            credential_id
                .map(ProviderCredentialId::as_str)
                .and_then(deepseek_session_path),
        );
        return PacingScope::Session(path);
    }
    if is_perplexity_web(provider) {
        let path = session_scope_key(
            credential_id,
            credential_id
                .map(ProviderCredentialId::as_str)
                .and_then(perplexity_session_path),
        );
        return PacingScope::Session(path);
    }
    let credential = credential_id
        .map_or_else(|| "default".into(), |id| id.as_str().to_string());
    if quota_profile == ProviderQuotaProfile::PerModel
        && let Some(model) = model
    {
        return PacingScope::CredentialModel {
            credential,
            model: model.to_string(),
        };
    }
    PacingScope::Credential(credential)
}

fn session_scope_key(
    credential_id: Option<&ProviderCredentialId>,
    session_path: Option<std::path::PathBuf>,
) -> String {
    session_path.map_or_else(
        || {
            credential_id.map_or_else(
                || "missing-session".into(),
                |id| format!("credential:{}", id.as_str()),
            )
        },
        |path| path.display().to_string(),
    )
}

#[must_use]
pub fn pacing_scope_key(scope: &PacingScope) -> String {
    match scope {
        PacingScope::Session(path) | PacingScope::Credential(path) => {
            path.clone()
        }
        PacingScope::CredentialModel { credential, model } => {
            format!("{credential}::{model}")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::credentials::ProviderCredentialId;

    #[test]
    fn api_provider_uses_credential_scope() {
        let provider = InferenceProvider::GoogleGemini;
        let id = ProviderCredentialId::new("gemini-free");
        let scope = resolve_pacing_scope(
            &provider,
            Some(&id),
            None,
            ProviderQuotaProfile::PerSlot,
        );
        assert_eq!(pacing_scope_key(&scope), "gemini-free");
    }

    #[test]
    fn per_model_scope_includes_model_slug() {
        let provider = InferenceProvider::GoogleGemini;
        let id = ProviderCredentialId::new("gemini-free-8");
        let scope = resolve_pacing_scope(
            &provider,
            Some(&id),
            Some("gemini-3-flash-preview"),
            ProviderQuotaProfile::PerModel,
        );
        assert_eq!(
            pacing_scope_key(&scope),
            "gemini-free-8::gemini-3-flash-preview"
        );
    }

    #[test]
    fn openrouter_per_model_scope_isolates_slugs_on_same_credential() {
        let provider = InferenceProvider::OpenRouter;
        let id = ProviderCredentialId::new("openrouter-default");
        let nemotron = resolve_pacing_scope(
            &provider,
            Some(&id),
            Some("nvidia/nemotron-3-nano-30b-a3b:free"),
            ProviderQuotaProfile::PerModel,
        );
        let gpt_oss = resolve_pacing_scope(
            &provider,
            Some(&id),
            Some("openai/gpt-oss-120b:free"),
            ProviderQuotaProfile::PerModel,
        );
        assert_ne!(pacing_scope_key(&nemotron), pacing_scope_key(&gpt_oss));
    }

    #[test]
    #[serial_test::serial]
    fn deepseek_scope_uses_session_path() {
        let path = std::env::temp_dir().join("ai-gw-ds-scope.json");
        std::fs::write(&path, r#"{"token":"tok"}"#).unwrap();
        let mut secrets = crate::config::secrets_file::SecretsFile::default();
        secrets.register_session_path("deepseek-web-default", path.clone());
        let _guard =
            crate::config::secrets_file::SecretsFile::install_for_test(secrets);

        let provider = InferenceProvider::Named("deepseek-web".into());
        let id = ProviderCredentialId::new("deepseek-web-default");
        let scope = resolve_pacing_scope(
            &provider,
            Some(&id),
            Some("deepseek-chat"),
            ProviderQuotaProfile::PerSession,
        );
        assert_eq!(pacing_scope_key(&scope), path.display().to_string());
        let _ = std::fs::remove_file(path);
    }
}
