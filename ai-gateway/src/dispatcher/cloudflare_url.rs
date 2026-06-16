use url::Url;

use crate::{
    error::internal::InternalError, types::provider::InferenceProvider,
};

pub fn join_provider_path(
    provider: &InferenceProvider,
    base_url: &Url,
    path: &str,
    cloudflare_account_id: Option<&str>,
) -> Result<Url, InternalError> {
    let base = cloudflare_ai_base(provider, base_url, cloudflare_account_id)?;
    base.join(path).map_err(|error| {
        tracing::error!(%error, provider = %provider, path, "invalid provider URL join");
        InternalError::Internal
    })
}

fn cloudflare_ai_base(
    provider: &InferenceProvider,
    base_url: &Url,
    cloudflare_account_id: Option<&str>,
) -> Result<Url, InternalError> {
    if provider != &InferenceProvider::Named("cloudflare".into()) {
        return Ok(base_url.clone());
    }
    let account_id = cloudflare_account_id.ok_or_else(|| {
        tracing::error!(
            "cloudflare account id missing — set \
             credentials.cloudflare-default in secrets file"
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
            None,
        )
        .unwrap();
        assert_eq!(
            joined.as_str(),
            "https://api.cerebras.ai/v1/chat/completions"
        );
    }

    #[test]
    fn cloudflare_joins_account_scoped_base() {
        let base =
            url::Url::parse("https://api.cloudflare.com/client/v4/accounts/")
                .unwrap();
        let joined = join_provider_path(
            &InferenceProvider::Named("cloudflare".into()),
            &base,
            "v1/chat/completions",
            Some("acct123"),
        )
        .unwrap();
        assert!(joined.as_str().contains("acct123/ai/"));
    }
}
