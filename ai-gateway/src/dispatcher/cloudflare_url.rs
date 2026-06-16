use url::Url;

use crate::{
    error::internal::InternalError, types::provider::InferenceProvider,
};

pub fn join_provider_path(
    provider: &InferenceProvider,
    base_url: &Url,
    path: &str,
) -> Result<Url, InternalError> {
    let base = cloudflare_ai_base(provider, base_url)?;
    base.join(path).map_err(|error| {
        tracing::error!(%error, provider = %provider, path, "invalid provider URL join");
        InternalError::Internal
    })
}

fn cloudflare_ai_base(
    provider: &InferenceProvider,
    base_url: &Url,
) -> Result<Url, InternalError> {
    if provider != &InferenceProvider::Named("cloudflare".into()) {
        return Ok(base_url.clone());
    }
    let account_id = crate::config::cloudflare::credentials_from_env()
        .map(|(account_id, _)| account_id)
        .ok_or_else(|| {
            tracing::error!(
                "CLOUDFLARE_API_KEY_WITH_ACCOUNT_ID or CLOUDFLARE_ACCOUNT_ID \
                 is required when cloudflare provider is configured"
            );
            InternalError::ProviderNotConfigured(provider.clone())
        })?;
    base_url.join(&format!("{account_id}/ai/")).map_err(|error| {
        tracing::error!(%error, account_id, "invalid cloudflare base URL join");
        InternalError::Internal
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::provider::InferenceProvider;

    #[test]
    fn cerebras_base_url_joins_openai_chat_path_once() {
        let base = url::Url::parse("https://api.cerebras.ai/").unwrap();
        let joined = join_provider_path(
            &InferenceProvider::Named("cerebras".into()),
            &base,
            "v1/chat/completions",
        )
        .unwrap();
        assert_eq!(
            joined.as_str(),
            "https://api.cerebras.ai/v1/chat/completions"
        );
    }

    #[test]
    fn github_models_base_url_joins_inference_chat_completions() {
        let base =
            url::Url::parse("https://models.github.ai/inference/").unwrap();
        let joined = join_provider_path(
            &InferenceProvider::Named("github-models".into()),
            &base,
            "chat/completions",
        )
        .unwrap();
        assert_eq!(
            joined.as_str(),
            "https://models.github.ai/inference/chat/completions"
        );
    }
}
