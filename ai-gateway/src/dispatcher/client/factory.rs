use super::Client;
use crate::{
    app_state::AppState,
    dispatcher::{
        anthropic_client::Client as AnthropicClient,
        bedrock_client::Client as BedrockClient,
        ollama_client::Client as OllamaClient,
        openai_compatible_client::Client as OpenAICompatibleClient,
    },
    error::init::InitError,
    types::provider::{InferenceProvider, ProviderKey},
};

impl Client {
    pub async fn new(
        app_state: &AppState,
        provider: InferenceProvider,
    ) -> Result<Self, InitError> {
        let api_key = if provider.is_keyless() {
            None
        } else if let Some(key) =
            app_state.config().credentials.default_key(&provider)
        {
            Some(key)
        } else {
            app_state
                .0
                .provider_keys
                .get_provider_key(&provider, None)
                .await
        };
        Self::new_with_provider_key(app_state, provider, api_key.as_ref())
    }

    pub fn new_with_provider_key(
        app_state: &AppState,
        provider: InferenceProvider,
        provider_key: Option<&ProviderKey>,
    ) -> Result<Self, InitError> {
        Self::new_inner(app_state, provider, provider_key)
    }

    fn new_inner(
        app_state: &AppState,
        provider: InferenceProvider,
        api_key: Option<&ProviderKey>,
    ) -> Result<Self, InitError> {
        let gzip = app_state.config().gzip_decompress_responses_for(&provider);
        let d = &app_state.config().dispatcher;
        let base = reqwest::Client::builder()
            .gzip(gzip)
            .connect_timeout(d.connection_timeout)
            .timeout(d.timeout)
            .tcp_nodelay(true);
        match provider {
            InferenceProvider::OpenAI
            | InferenceProvider::GoogleGemini
            | InferenceProvider::OpenRouter
            | InferenceProvider::Named(_) => {
                Ok(Self::OpenAICompatible(OpenAICompatibleClient::new(
                    app_state, base, provider, api_key,
                )?))
            }
            InferenceProvider::Anthropic => Ok(Self::Anthropic(
                AnthropicClient::new(app_state, base, api_key)?,
            )),
            InferenceProvider::Bedrock => {
                Ok(Self::Bedrock(BedrockClient::new(app_state, base, api_key)?))
            }
            InferenceProvider::Ollama => {
                Ok(Self::Ollama(OllamaClient::new(app_state, base)?))
            }
        }
    }
}
