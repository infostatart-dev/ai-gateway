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
        let api_key = if provider == InferenceProvider::Ollama {
            None
        } else {
            Some(
                app_state
                    .0
                    .provider_keys
                    .get_provider_key(&provider, None)
                    .await,
            )
        };
        Self::new_inner(
            app_state,
            provider,
            api_key.as_ref().and_then(|k| k.as_ref()),
        )
    }

    fn new_inner(
        app_state: &AppState,
        provider: InferenceProvider,
        api_key: Option<&ProviderKey>,
    ) -> Result<Self, InitError> {
        let base = reqwest::Client::builder()
            .connect_timeout(app_state.0.config.dispatcher.connection_timeout)
            .timeout(app_state.0.config.dispatcher.timeout)
            .tcp_nodelay(true)
            // Enable transparent gzip decompression for upstream provider
            // responses. The `gzip` feature is already enabled in the
            // workspace Cargo.toml; without `.gzip(true)` reqwest sends
            // `Accept-Encoding: gzip` only if the user does so manually.
            // OpenRouter sometimes responds with gzipped bodies, and
            // without decompression the downstream JSON deserializer fails
            // with "expected value at line 1 column 1" on the magic bytes.
            .gzip(true);
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
