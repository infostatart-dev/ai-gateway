use std::sync::Arc;

use rustc_hash::FxHashMap as HashMap;

use super::{
    EndpointConverter, TypedEndpointConverter, anthropic::AnthropicConverter,
    model::ModelMapper, openai::OpenAIConverter,
    openai_compatible::OpenAICompatibleConverter,
    openrouter::OpenRouterConverter,
};
use crate::{
    endpoints::{
        self, ApiEndpoint, anthropic::Anthropic, bedrock::Bedrock,
        google::Google, ollama::Ollama, openai::OpenAI, openrouter::OpenRouter,
    },
    middleware::mapper::{bedrock::BedrockConverter, ollama::OllamaConverter},
    types::provider::InferenceProvider,
};

#[derive(Debug, Default, Clone)]
pub struct EndpointConverterRegistry(Arc<EndpointConverterRegistryInner>);

impl EndpointConverterRegistry {
    #[must_use]
    pub fn new(model_mapper: &ModelMapper) -> Self {
        let inner = EndpointConverterRegistryInner::new(model_mapper);
        Self(Arc::new(inner))
    }

    #[must_use]
    pub fn get_converter(
        &self,
        source_endpoint: &ApiEndpoint,
        target_endpoint: &ApiEndpoint,
    ) -> Option<&(dyn EndpointConverter + Send + Sync + 'static)> {
        self.0
            .converters
            .get(&RegistryKey::new(
                source_endpoint.clone(),
                target_endpoint.clone(),
            ))
            .map(|v| &**v)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct RegistryKey {
    source_endpoint: ApiEndpoint,
    target_endpoint: ApiEndpoint,
}

impl RegistryKey {
    fn new(source_endpoint: ApiEndpoint, target_endpoint: ApiEndpoint) -> Self {
        Self {
            source_endpoint,
            target_endpoint,
        }
    }
}

#[derive(Default)]
struct EndpointConverterRegistryInner {
    /// In the future when we support other APIs beside just chat completion
    /// we'll want to add another level here.
    converters: HashMap<
        RegistryKey,
        Box<dyn EndpointConverter + Send + Sync + 'static>,
    >,
}

impl std::fmt::Debug for EndpointConverterRegistryInner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut debug = f.debug_struct("EndpointConverterRegistryInner");
        debug.field("converters", &self.converters.keys().collect::<Vec<_>>());
        debug.finish()
    }
}

impl EndpointConverterRegistryInner {
    #[allow(clippy::too_many_lines)]
    fn new(model_mapper: &ModelMapper) -> Self {
        let mut registry = Self {
            converters: HashMap::default(),
        };

        let key = RegistryKey::new(
            ApiEndpoint::OpenAI(OpenAI::chat_completions()),
            ApiEndpoint::Anthropic(Anthropic::messages()),
        );
        let converter =
            TypedEndpointConverter::<
                endpoints::openai::ChatCompletions,
                endpoints::anthropic::Messages,
                AnthropicConverter,
            >::new(AnthropicConverter::new(model_mapper.clone()));
        registry.register_converter(key, converter);

        let key = RegistryKey::new(
            ApiEndpoint::OpenAI(OpenAI::chat_completions()),
            ApiEndpoint::Google(Google::generate_contents()),
        );
        let converter = TypedEndpointConverter::<
            endpoints::openai::ChatCompletions,
            endpoints::google::GenerateContents,
            OpenAICompatibleConverter,
        >::new(OpenAICompatibleConverter::new(
            InferenceProvider::GoogleGemini,
            model_mapper.clone(),
        ));
        registry.register_converter(key, converter);

        let key = RegistryKey::new(
            ApiEndpoint::OpenAI(OpenAI::chat_completions()),
            ApiEndpoint::OpenAI(OpenAI::chat_completions()),
        );
        let converter =
            TypedEndpointConverter::<
                endpoints::openai::ChatCompletions,
                endpoints::openai::ChatCompletions,
                OpenAIConverter,
            >::new(OpenAIConverter::new(model_mapper.clone()));
        registry.register_converter(key, converter);

        let key = RegistryKey::new(
            ApiEndpoint::OpenAI(OpenAI::chat_completions()),
            ApiEndpoint::Ollama(Ollama::chat_completions()),
        );
        let converter =
            TypedEndpointConverter::<
                endpoints::openai::ChatCompletions,
                endpoints::ollama::chat_completions::ChatCompletions,
                OllamaConverter,
            >::new(OllamaConverter::new(model_mapper.clone()));
        registry.register_converter(key, converter);

        let key = RegistryKey::new(
            ApiEndpoint::OpenAI(OpenAI::chat_completions()),
            ApiEndpoint::Bedrock(Bedrock::converse()),
        );

        let converter =
            TypedEndpointConverter::<
                endpoints::openai::ChatCompletions,
                endpoints::bedrock::Converse,
                BedrockConverter,
            >::new(BedrockConverter::new(model_mapper.clone()));

        registry.register_converter(key, converter);

        let key = RegistryKey::new(
            ApiEndpoint::OpenAI(OpenAI::chat_completions()),
            ApiEndpoint::OpenRouter(OpenRouter::chat_completions()),
        );
        let converter =
            TypedEndpointConverter::<
                endpoints::openai::ChatCompletions,
                endpoints::openrouter::chat_completions::ChatCompletions,
                OpenRouterConverter,
            >::new(OpenRouterConverter::new(model_mapper.clone()));
        registry.register_converter(key, converter);

        let key = RegistryKey::new(
            ApiEndpoint::OpenAI(OpenAI::chat_completions()),
            ApiEndpoint::OpenAICompatible {
                provider: InferenceProvider::Named("mistral".into()),
                openai_endpoint: OpenAI::chat_completions(),
            },
        );
        let converter = TypedEndpointConverter::<
            endpoints::openai::ChatCompletions,
            endpoints::openai::OpenAICompatibleChatCompletions,
            OpenAICompatibleConverter,
        >::new(OpenAICompatibleConverter::new(
            InferenceProvider::Named("mistral".into()),
            model_mapper.clone(),
        ));
        registry.register_converter(key, converter);

        let key = RegistryKey::new(
            ApiEndpoint::OpenAI(OpenAI::chat_completions()),
            ApiEndpoint::OpenAICompatible {
                provider: InferenceProvider::Named("groq".into()),
                openai_endpoint: OpenAI::chat_completions(),
            },
        );
        let converter = TypedEndpointConverter::<
            endpoints::openai::ChatCompletions,
            super::groq::GroqChatCompletions,
            super::groq::GroqConverter,
        >::new(super::groq::GroqConverter::new(
            model_mapper.clone(),
        ));
        registry.register_converter(key, converter);

        let key = RegistryKey::new(
            ApiEndpoint::OpenAI(OpenAI::chat_completions()),
            ApiEndpoint::OpenAICompatible {
                provider: InferenceProvider::Named("cerebras".into()),
                openai_endpoint: OpenAI::chat_completions(),
            },
        );
        let converter = TypedEndpointConverter::<
            endpoints::openai::ChatCompletions,
            endpoints::openai::OpenAICompatibleChatCompletions,
            OpenAICompatibleConverter,
        >::new(OpenAICompatibleConverter::new(
            InferenceProvider::Named("cerebras".into()),
            model_mapper.clone(),
        ));
        registry.register_converter(key, converter);

        let key = RegistryKey::new(
            ApiEndpoint::OpenAI(OpenAI::chat_completions()),
            ApiEndpoint::OpenAICompatible {
                provider: InferenceProvider::Named("deepseek".into()),
                openai_endpoint: OpenAI::chat_completions(),
            },
        );
        let converter = TypedEndpointConverter::<
            endpoints::openai::ChatCompletions,
            endpoints::openai::OpenAICompatibleChatCompletions,
            OpenAICompatibleConverter,
        >::new(OpenAICompatibleConverter::new(
            InferenceProvider::Named("deepseek".into()),
            model_mapper.clone(),
        ));
        registry.register_converter(key, converter);

        let key = RegistryKey::new(
            ApiEndpoint::OpenAI(OpenAI::chat_completions()),
            ApiEndpoint::OpenAICompatible {
                provider: InferenceProvider::Named("xai".into()),
                openai_endpoint: OpenAI::chat_completions(),
            },
        );
        let converter = TypedEndpointConverter::<
            endpoints::openai::ChatCompletions,
            endpoints::openai::OpenAICompatibleChatCompletions,
            OpenAICompatibleConverter,
        >::new(OpenAICompatibleConverter::new(
            InferenceProvider::Named("xai".into()),
            model_mapper.clone(),
        ));
        registry.register_converter(key, converter);

        let key = RegistryKey::new(
            ApiEndpoint::OpenAI(OpenAI::chat_completions()),
            ApiEndpoint::OpenAICompatible {
                provider: InferenceProvider::Named("opencode".into()),
                openai_endpoint: OpenAI::chat_completions(),
            },
        );
        let converter = TypedEndpointConverter::<
            endpoints::openai::ChatCompletions,
            endpoints::openai::OpenAICompatibleChatCompletions,
            OpenAICompatibleConverter,
        >::new(OpenAICompatibleConverter::new(
            InferenceProvider::Named("opencode".into()),
            model_mapper.clone(),
        ));
        registry.register_converter(key, converter);

        let key = RegistryKey::new(
            ApiEndpoint::OpenAI(OpenAI::chat_completions()),
            ApiEndpoint::OpenAICompatible {
                provider: InferenceProvider::Named("cloudflare".into()),
                openai_endpoint: OpenAI::chat_completions(),
            },
        );
        let converter = TypedEndpointConverter::<
            endpoints::openai::ChatCompletions,
            super::cloudflare::CloudflareChatCompletions,
            super::cloudflare::CloudflareConverter,
        >::new(
            super::cloudflare::CloudflareConverter::new(model_mapper.clone()),
        );
        registry.register_converter(key, converter);

        let key = RegistryKey::new(
            ApiEndpoint::OpenAI(OpenAI::chat_completions()),
            ApiEndpoint::OpenAICompatible {
                provider: InferenceProvider::Named("hyperbolic".into()),
                openai_endpoint: OpenAI::chat_completions(),
            },
        );
        let converter = TypedEndpointConverter::<
            endpoints::openai::ChatCompletions,
            endpoints::openai::OpenAICompatibleChatCompletions,
            OpenAICompatibleConverter,
        >::new(OpenAICompatibleConverter::new(
            InferenceProvider::Named("hyperbolic".into()),
            model_mapper.clone(),
        ));
        registry.register_converter(key, converter);

        let key = RegistryKey::new(
            ApiEndpoint::OpenAI(OpenAI::chat_completions()),
            ApiEndpoint::OpenAICompatible {
                provider: InferenceProvider::Named("chatgpt-web".into()),
                openai_endpoint: OpenAI::chat_completions(),
            },
        );
        let converter = TypedEndpointConverter::<
            endpoints::openai::ChatCompletions,
            super::chatgpt_web::ChatGptWebChatCompletions,
            super::chatgpt_web::ChatGptWebConverter,
        >::new(
            super::chatgpt_web::ChatGptWebConverter::new(model_mapper.clone()),
        );
        registry.register_converter(key, converter);

        let key = RegistryKey::new(
            ApiEndpoint::OpenAI(OpenAI::chat_completions()),
            ApiEndpoint::OpenAICompatible {
                provider: InferenceProvider::Named("deepseek-web".into()),
                openai_endpoint: OpenAI::chat_completions(),
            },
        );
        let converter =
            TypedEndpointConverter::<
                endpoints::openai::ChatCompletions,
                super::deepseek_web::DeepSeekWebChatCompletions,
                super::deepseek_web::DeepSeekWebConverter,
            >::new(super::deepseek_web::DeepSeekWebConverter::new(
                model_mapper.clone(),
            ));
        registry.register_converter(key, converter);

        let key = RegistryKey::new(
            ApiEndpoint::OpenAI(OpenAI::chat_completions()),
            ApiEndpoint::OpenAICompatible {
                provider: InferenceProvider::Named("github-models".into()),
                openai_endpoint: OpenAI::chat_completions(),
            },
        );
        let converter = TypedEndpointConverter::<
            endpoints::openai::ChatCompletions,
            endpoints::openai::OpenAICompatibleChatCompletions,
            OpenAICompatibleConverter,
        >::new(OpenAICompatibleConverter::new(
            InferenceProvider::Named("github-models".into()),
            model_mapper.clone(),
        ));
        registry.register_converter(key, converter);

        registry.register_catalog_named_compatible(model_mapper);

        registry
    }

    fn register_catalog_named_compatible(
        &mut self,
        model_mapper: &ModelMapper,
    ) {
        use crate::config::{
            provider_limits::ProviderLimitCatalog, providers::ProvidersConfig,
        };
        let providers = ProvidersConfig::default();
        let limits = ProviderLimitCatalog::default();
        let source = ApiEndpoint::OpenAI(OpenAI::chat_completions());
        self.register_named_api_key_providers(
            model_mapper,
            &providers,
            &limits,
            &source,
        );
    }

    fn register_named_api_key_providers(
        &mut self,
        model_mapper: &ModelMapper,
        providers: &crate::config::providers::ProvidersConfig,
        limits: &crate::config::provider_limits::ProviderLimitCatalog,
        source: &ApiEndpoint,
    ) {
        for (provider, _) in providers.iter() {
            let InferenceProvider::Named(_) = provider else {
                continue;
            };
            if limits
                .provider(provider)
                .and_then(|entry| entry.scope.as_deref())
                == Some("browser-session")
            {
                continue;
            }
            let target = ApiEndpoint::OpenAICompatible {
                provider: provider.clone(),
                openai_endpoint: OpenAI::chat_completions(),
            };
            let key = RegistryKey::new(source.clone(), target);
            if self.converters.contains_key(&key) {
                continue;
            }
            let converter = TypedEndpointConverter::<
                endpoints::openai::ChatCompletions,
                endpoints::openai::OpenAICompatibleChatCompletions,
                OpenAICompatibleConverter,
            >::new(OpenAICompatibleConverter::new(
                provider.clone(),
                model_mapper.clone(),
            ));
            self.register_converter(key, converter);
        }
    }

    #[cfg(test)]
    pub fn named_api_key_provider_keys(
        providers: &crate::config::providers::ProvidersConfig,
        limits: &crate::config::provider_limits::ProviderLimitCatalog,
    ) -> Vec<RegistryKey> {
        let source = ApiEndpoint::OpenAI(OpenAI::chat_completions());
        let mut keys = Vec::new();
        for (provider, _) in providers.iter() {
            let InferenceProvider::Named(_) = provider else {
                continue;
            };
            if limits
                .provider(provider)
                .and_then(|entry| entry.scope.as_deref())
                == Some("browser-session")
            {
                continue;
            }
            let target = ApiEndpoint::OpenAICompatible {
                provider: provider.clone(),
                openai_endpoint: OpenAI::chat_completions(),
            };
            keys.push(RegistryKey::new(source.clone(), target));
        }
        keys
    }

    fn register_converter<C>(&mut self, key: RegistryKey, converter: C)
    where
        C: EndpointConverter + Send + Sync + 'static,
    {
        self.converters.insert(key, Box::new(converter));
    }
}

#[cfg(test)]
mod catalog_registry_tests {
    use super::*;
    use crate::config::{
        provider_limits::ProviderLimitCatalog, providers::ProvidersConfig,
    };

    fn named_api_key_provider_in_catalog(name: &str) -> bool {
        let providers = ProvidersConfig::default();
        let limits = ProviderLimitCatalog::default();
        let keys = EndpointConverterRegistryInner::named_api_key_provider_keys(
            &providers, &limits,
        );
        let target = ApiEndpoint::OpenAICompatible {
            provider: InferenceProvider::Named(name.into()),
            openai_endpoint: OpenAI::chat_completions(),
        };
        let source = ApiEndpoint::OpenAI(OpenAI::chat_completions());
        keys.contains(&RegistryKey::new(source, target))
    }

    #[test]
    fn longcat_is_registered_as_named_api_key_provider() {
        assert!(
            named_api_key_provider_in_catalog("longcat"),
            "longcat not in catalog as api-key provider"
        );
    }

    #[test]
    fn bazaarlink_is_registered_as_named_api_key_provider() {
        assert!(
            named_api_key_provider_in_catalog("bazaarlink"),
            "bazaarlink not in catalog as api-key provider"
        );
    }
}
