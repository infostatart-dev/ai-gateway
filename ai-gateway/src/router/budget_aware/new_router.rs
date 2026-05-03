use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Duration,
};

use indexmap::IndexMap;
use nonempty_collections::NESet;

use super::types::{BudgetAwareRouter, BudgetCandidate};
use crate::{
    app_state::AppState,
    config::router::RouterConfig,
    dispatcher::Dispatcher,
    error::init::InitError,
    middleware::mapper::model::ModelMapper,
    router::capability::get_model_capability,
    types::{provider::InferenceProvider, router::RouterId},
};

impl BudgetAwareRouter {
    pub async fn new(
        app_state: AppState,
        router_id: RouterId,
        router_config: Arc<RouterConfig>,
        providers: &NESet<InferenceProvider>,
        provider_priorities: &IndexMap<InferenceProvider, u16>,
        max_cooldown_wait: Duration,
    ) -> Result<Self, InitError> {
        let mut candidates = Vec::new();
        let providers_config = &app_state.config().providers;

        for provider in providers {
            if let Some(config) = providers_config.get(provider) {
                for model in &config.models {
                    let capability = get_model_capability(provider, model);
                    let service =
                        Dispatcher::new_with_model_id_without_rate_limit_events(
                            app_state.clone(),
                            &router_id,
                            &router_config,
                            provider.clone(),
                            model.clone(),
                        )
                        .await?;

                    candidates.push(BudgetCandidate {
                        capability,
                        service,
                    });
                }
            }
        }

        Ok(Self {
            candidates: Arc::new(candidates),
            model_mapper: ModelMapper::new_for_router(
                app_state.clone(),
                router_config,
            ),
            states: Arc::new(Mutex::new(HashMap::new())),
            provider_priorities: Arc::new(provider_priorities.clone()),
            default_latency: app_state.config().discover.default_rtt,
            max_cooldown_wait,
        })
    }
}
