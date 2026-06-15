use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Duration,
};

use indexmap::IndexMap;
use nonempty_collections::NESet;

use super::credential_balance::CredentialRoundRobin;
use super::types::{
    BudgetAwareRouter, BudgetCandidate, CandidateSelectionMode,
};
use crate::{
    app_state::AppState,
    config::{
        credentials::ProviderCredentialId, providers::GlobalProviderConfig,
        router::RouterConfig,
    },
    dispatcher::Dispatcher,
    endpoints::EndpointType,
    error::init::InitError,
    middleware::mapper::model::ModelMapper,
    router::capability::get_model_capability,
    types::{provider::InferenceProvider, router::RouterId},
};

async fn push_anonymous_candidates(
    candidates: &mut Vec<BudgetCandidate>,
    app_state: AppState,
    router_id: &RouterId,
    router_config: &Arc<RouterConfig>,
    provider: &InferenceProvider,
    config: &GlobalProviderConfig,
) -> Result<(), InitError> {
    let credential_id =
        ProviderCredentialId::new(format!("{provider}-anonymous"));
    let credential_budget_rank = 0;

    for model in &config.models {
        let capability = get_model_capability(
            provider,
            model,
            config.model_capabilities.get(model),
        );
        let service = Dispatcher::new_with_model_id_and_provider_key_without_rate_limit_events(
            app_state.clone(),
            router_id,
            router_config,
            provider.clone(),
            model.clone(),
            None,
            Some(&credential_id),
        )
        .await?;
        candidates.push(BudgetCandidate {
            credential_id: credential_id.clone(),
            credential_budget_rank,
            capability,
            service,
        });
    }
    Ok(())
}

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
    let credentials = &app_state.config().credentials;

    for provider in providers {
        let Some(config) = providers_config.get(provider) else {
            continue;
        };
        let provider_credentials: Vec<_> =
            credentials.for_provider(provider).collect();

        if provider_credentials.is_empty() {
            push_anonymous_candidates(
                &mut candidates,
                app_state.clone(),
                &router_id,
                &router_config,
                provider,
                config,
            )
            .await?;
            continue;
        }

        for credential in provider_credentials {
            for model in &config.models {
                let capability = get_model_capability(
                    provider,
                    model,
                    config.model_capabilities.get(model),
                );
                let service = Dispatcher::new_with_model_id_and_provider_key_without_rate_limit_events(
                    app_state.clone(),
                    &router_id,
                    &router_config,
                    provider.clone(),
                    model.clone(),
                    Some(&credential.key),
                    Some(&credential.id),
                )
                .await?;
                candidates.push(BudgetCandidate {
                    credential_id: credential.id.clone(),
                    credential_budget_rank: credential.budget_rank,
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
        credential_round_robin: CredentialRoundRobin::new_shared(),
    })
}
