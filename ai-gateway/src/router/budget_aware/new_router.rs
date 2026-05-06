use std::{sync::Arc, time::Duration};

use indexmap::IndexMap;
use nonempty_collections::NESet;

use super::{
    factory,
    types::{BudgetAwareRouter, CandidateSelectionMode},
};
use crate::{
    app_state::AppState,
    config::router::RouterConfig,
    error::init::InitError,
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
        factory::build(
            app_state,
            router_id,
            router_config,
            providers,
            provider_priorities,
            max_cooldown_wait,
            CandidateSelectionMode::CapabilityThenBudget,
        )
        .await
    }

    pub async fn new_budget_then_capability(
        app_state: AppState,
        router_id: RouterId,
        router_config: Arc<RouterConfig>,
        providers: &NESet<InferenceProvider>,
        provider_priorities: &IndexMap<InferenceProvider, u16>,
        max_cooldown_wait: Duration,
    ) -> Result<Self, InitError> {
        factory::build(
            app_state,
            router_id,
            router_config,
            providers,
            provider_priorities,
            max_cooldown_wait,
            CandidateSelectionMode::BudgetThenCapability,
        )
        .await
    }
}
