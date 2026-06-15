use crate::{
    config::{
        chatgpt_web::{is_chatgpt_web, session_path_for_credential as chatgpt_session_path},
        credentials::ProviderCredentialId,
        perplexity_web::{is_perplexity_web, session_path_for_credential as perplexity_session_path},
    },
    types::provider::InferenceProvider,
};

#[must_use]
pub fn gate_scope_key(
    provider: &InferenceProvider,
    credential_id: Option<&ProviderCredentialId>,
) -> String {
    if is_chatgpt_web(provider) {
        return credential_id
            .map(|id| id.0.as_str())
            .and_then(chatgpt_session_path)
            .map_or_else(
                || "missing-session".into(),
                |path| path.display().to_string(),
            );
    }
    if is_perplexity_web(provider) {
        return credential_id
            .map(|id| id.0.as_str())
            .and_then(perplexity_session_path)
            .map_or_else(
                || "missing-session".into(),
                |path| path.display().to_string(),
            );
    }
    credential_id.map_or_else(|| "default".into(), ToString::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::credentials::ProviderCredentialId;

    #[test]
    fn api_provider_uses_credential_scope() {
        let provider = InferenceProvider::GoogleGemini;
        let id = ProviderCredentialId::new("gemini-free");
        assert_eq!(gate_scope_key(&provider, Some(&id)), "gemini-free");
        assert_eq!(gate_scope_key(&provider, None), "default");
    }
}
