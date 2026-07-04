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
        for provider in managed_providers() {
            if !app_state.config().providers.contains_key(&provider) {
                continue;
            }
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

fn managed_providers() -> Vec<InferenceProvider> {
    vec![
        InferenceProvider::Named("chatgpt-web".into()),
        InferenceProvider::Named("deepseek-web".into()),
        InferenceProvider::Named("longcat".into()),
        InferenceProvider::Named("llm7".into()),
        InferenceProvider::GoogleGemini,
        InferenceProvider::OpenAI,
    ]
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
    use super::*;

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
    fn managed_mode_is_enabled_for_browser_and_api_upstreams() {
        let providers = managed_providers();
        assert_eq!(providers.len(), 6);
        assert!(
            providers.contains(&InferenceProvider::Named("chatgpt-web".into()))
        );
        assert!(
            providers
                .contains(&InferenceProvider::Named("deepseek-web".into()))
        );
        assert!(
            providers.contains(&InferenceProvider::Named("longcat".into()))
        );
        assert!(providers.contains(&InferenceProvider::Named("llm7".into())));
        assert!(providers.contains(&InferenceProvider::GoogleGemini));
        assert!(providers.contains(&InferenceProvider::OpenAI));
    }
}
