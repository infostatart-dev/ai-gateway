use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Duration,
};

use indexmap::IndexMap;
use nonempty_collections::NESet;

use super::types::{
    BudgetAwareRouter, BudgetCandidate, CandidateSelectionMode,
};
use crate::{
    app_state::AppState,
    config::router::RouterConfig,
    dispatcher::Dispatcher,
    endpoints::EndpointType,
    error::init::InitError,
    middleware::mapper::model::ModelMapper,
    router::capability::get_model_capability,
    types::{provider::InferenceProvider, router::RouterId},
};

#[allow(clippy::too_many_arguments)]
pub(super) async fn build(
    app_state: AppState,
    router_id: RouterId,
    router_config: Arc<RouterConfig>,
    providers: &NESet<InferenceProvider>,
    provider_priorities: &IndexMap<InferenceProvider, u16>,
    max_cooldown_wait: Duration,
    selection_mode: CandidateSelectionMode,
    endpoint_type: EndpointType,
    strategy: &'static str,
) -> Result<BudgetAwareRouter, InitError> {
    let mut candidates = Vec::new();
    let providers_config = &app_state.config().providers;

    for provider in providers {
        if let Some(config) = providers_config.get(provider) {
            for model in &config.models {
                let capability = get_model_capability(
                    provider,
                    model,
                    config.model_capabilities.get(model),
                );
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

    let model_mapper =
        ModelMapper::new_for_router(app_state.clone(), router_config);
    let default_latency = app_state.config().discover.default_rtt;
    Ok(BudgetAwareRouter {
        app_state,
        router_id,
        endpoint_type,
        strategy,
        candidates: Arc::new(candidates),
        model_mapper,
        states: Arc::new(Mutex::new(HashMap::new())),
        provider_priorities: Arc::new(provider_priorities.clone()),
        default_latency,
        max_cooldown_wait,
        selection_mode,
    })
}
