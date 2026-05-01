use crate::{
    app_state::AppState,
    dispatcher::{
        anthropic_client::Client as AnthropicClient,
        bedrock_client::Client as BedrockClient,
        ollama_client::Client as OllamaClient,
        openai_compatible_client::Client as OpenAICompatibleClient,
    },
    error::api::ApiError,
    types::{extensions::AuthContext, provider::InferenceProvider},
};

pub mod auth;
pub mod factory;
pub mod metrics;
pub mod stream;

pub trait ProviderClient {
    async fn authenticate(
        &self,
        app_state: &AppState,
        request_builder: reqwest::RequestBuilder,
        req_body_bytes: &bytes::Bytes,
        auth_ctx: Option<&AuthContext>,
        provider: InferenceProvider,
    ) -> Result<reqwest::RequestBuilder, ApiError>;
}

#[derive(Debug, Clone)]
pub enum Client {
    OpenAICompatible(OpenAICompatibleClient),
    Anthropic(AnthropicClient),
    Ollama(OllamaClient),
    Bedrock(BedrockClient),
}

impl AsRef<reqwest::Client> for Client {
    fn as_ref(&self) -> &reqwest::Client {
        match self {
            Client::OpenAICompatible(c) => &c.0,
            Client::Anthropic(c) => &c.0,
            Client::Ollama(c) => &c.0,
            Client::Bedrock(c) => &c.inner,
        }
    }
}
