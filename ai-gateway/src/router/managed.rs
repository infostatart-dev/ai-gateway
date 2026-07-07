use std::{collections::HashMap, sync::Arc};

use indexmap::IndexMap;
use tower::ServiceBuilder;

use crate::{
    app_state::AppState,
    config::{
        balance::{
            BalanceConfig, BalanceConfigInner, default_budget_max_cooldown_wait,
        },
        router::{RouterConfig, SourceModelSelection},
    },
    endpoints::EndpointType,
    error::init::InitError,
    middleware::cache::optional::Layer as CacheLayer,
    router::service::Router,
    types::{provider::InferenceProvider, router::RouterId},
    utils::handle_error::ErrorHandlerLayer,
};

pub type ManagedUpstreamService =
    crate::middleware::cache::optional::Service<ErrorHandler<Router>>;

use crate::utils::handle_error::ErrorHandler;

#[derive(Debug)]
pub struct ManagedUpstreams {
    services: HashMap<InferenceProvider, ManagedUpstreamService>,
}

impl ManagedUpstreams {
    pub async fn new(app_state: &AppState) -> Result<Self, InitError> {
        let mut services = HashMap::new();
        let providers =
            configured_managed_providers(&app_state.config().providers);
        for provider in providers {
            let router_id = managed_router_id(&provider);
            let router_config = Arc::new(managed_router_config(&provider));
            let router =
                Router::new(router_id, router_config, app_state.clone())
                    .await?;
            let service = ServiceBuilder::new()
                .layer(CacheLayer::global(app_state)?)
                .layer(ErrorHandlerLayer::new(app_state.clone()))
                .service(router);
            services.insert(provider, service);
        }
        Ok(Self { services })
    }

    pub fn get(
        &self,
        provider: &InferenceProvider,
    ) -> Option<&ManagedUpstreamService> {
        self.services.get(provider)
    }
}

fn configured_managed_providers(
    providers: &crate::config::providers::ProvidersConfig,
) -> Vec<InferenceProvider> {
    providers.keys().cloned().collect()
}

fn managed_router_id(provider: &InferenceProvider) -> RouterId {
    RouterId::Named(format!("managed-{provider}").into())
}

fn managed_router_config(provider: &InferenceProvider) -> RouterConfig {
    let providers = nonempty_collections::nes![provider.clone()];
    let mut provider_priorities = IndexMap::new();
    provider_priorities.insert(provider.clone(), 0);

    RouterConfig {
        load_balance: BalanceConfig(HashMap::from([(
            EndpointType::Chat,
            BalanceConfigInner::BudgetAwareCapabilityAfter {
                providers,
                provider_priorities,
                max_cooldown_wait: default_budget_max_cooldown_wait(),
            },
        )])),
        source_model_selection: Some(SourceModelSelection::Strict),
        ..RouterConfig::default()
    }
}

#[cfg(test)]
mod tests {
    use indexmap::{IndexMap, IndexSet};
    use url::Url;

    use super::*;
    use crate::{
        config::providers::{GlobalProviderConfig, ProvidersConfig},
        types::model_id::ModelId,
    };

    #[test]
    fn managed_router_config_pins_single_provider() {
        let provider = InferenceProvider::Named("llm7".into());
        let config = managed_router_config(&provider);
        let strategy = config.load_balance.0.get(&EndpointType::Chat).unwrap();
        let BalanceConfigInner::BudgetAwareCapabilityAfter {
            providers,
            provider_priorities,
            ..
        } = strategy
        else {
            panic!("expected budget-aware managed strategy");
        };
        assert_eq!(providers.len().get(), 1);
        assert!(providers.contains(&provider));
        assert_eq!(provider_priorities.get(&provider), Some(&0));
    }

    #[test]
    fn managed_mode_uses_configured_providers_without_allowlist() {
        let custom = InferenceProvider::Named("custom-managed".into());
        let llm7 = InferenceProvider::Named("llm7".into());
        let providers = ProvidersConfig::from_iter([
            (custom.clone(), provider_config(&custom)),
            (llm7.clone(), provider_config(&llm7)),
        ]);

        assert_eq!(
            configured_managed_providers(&providers),
            vec![custom, llm7]
        );
    }

    #[cfg(feature = "testing")]
    #[tokio::test]
    async fn managed_upstreams_build_for_each_configured_provider() {
        use crate::{
            app::state::build_app_state, config::Config, tests::TestDefault,
        };

        let custom = InferenceProvider::Named("custom-managed".into());
        let llm7 = InferenceProvider::Named("llm7".into());
        let mut config = Config::test_default();
        config.providers = ProvidersConfig::from_iter([
            (custom.clone(), provider_config(&custom)),
            (llm7.clone(), provider_config(&llm7)),
        ]);

        let app_state = build_app_state(config).await.unwrap();
        let upstreams = ManagedUpstreams::new(&app_state).await.unwrap();

        assert_eq!(upstreams.services.len(), 2);
        assert!(upstreams.get(&custom).is_some());
        assert!(upstreams.get(&llm7).is_some());
    }

    fn provider_config(provider: &InferenceProvider) -> GlobalProviderConfig {
        let model = ModelId::from_str_and_provider(
            provider.clone(),
            "managed-test-model",
        )
        .unwrap();
        GlobalProviderConfig {
            models: IndexSet::from([model]),
            base_url: Url::parse("http://managed.test/").unwrap(),
            version: None,
            gzip_decompress_responses: None,
            model_capabilities: IndexMap::new(),
            request_headers: IndexMap::new(),
            model_catalog_keys: IndexMap::new(),
            last_verified_at: None,
            verify_source: None,
        }
    }
}
