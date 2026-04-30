use crate::{
    app_state::AppState,
    dispatcher::{anthropic_client::Client as AnthropicClient, openai_compatible_client::Client as OpenAICompatibleClient},
    error::{api::ApiError, auth::AuthError, internal::InternalError},
    types::{extensions::AuthContext, provider::{InferenceProvider, ProviderKey}},
};
use super::{Client, ProviderClient};

impl ProviderClient for Client {
    async fn authenticate(&self, app_state: &AppState, rb: reqwest::RequestBuilder, body: &bytes::Bytes, auth: Option<&AuthContext>, provider: InferenceProvider) -> Result<reqwest::RequestBuilder, ApiError> {
        match self {
            Client::Bedrock(inner) => inner.extract_and_sign_aws_headers(rb, body),
            Client::OpenAICompatible(_) | Client::Anthropic(_) => self.authenticate_inner(app_state, rb, auth, provider).await,
            Client::Ollama(_) => Ok(rb),
        }
    }
}

impl Client {
    async fn authenticate_inner(&self, app_state: &AppState, rb: reqwest::RequestBuilder, auth: Option<&AuthContext>, provider: InferenceProvider) -> Result<reqwest::RequestBuilder, ApiError> {
        if !app_state.config().deployment_target.is_cloud() { return Ok(rb); }
        let auth_ctx = auth.ok_or(ApiError::Authentication(AuthError::ProviderKeyNotFound))?;
        let org_id = auth_ctx.org_id;

        let key = if let Some(ProviderKey::Secret(k)) = app_state.0.provider_keys.get_provider_key(&provider, Some(&org_id)).await && k.expose() != "" { k } else {
            let keys = app_state.0.router_store.as_ref().ok_or(InternalError::Internal)?.get_org_provider_keys(org_id).await.map_err(|_| InternalError::Internal)?;
            app_state.set_org_provider_keys(org_id, keys.clone()).await;
            if let Some(ProviderKey::Secret(k)) = keys.get(&provider) { k.clone() } else { return Err(ApiError::Authentication(AuthError::ProviderKeyNotFound)); }
        };

        Ok(match self {
            Client::OpenAICompatible(_) => OpenAICompatibleClient::set_auth_header(rb, &key),
            Client::Anthropic(_) => AnthropicClient::set_auth_header(rb, &key),
            _ => rb,
        })
    }
}
