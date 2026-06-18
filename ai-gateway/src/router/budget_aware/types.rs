use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Duration,
};

use indexmap::IndexMap;

use super::credential_balance::CredentialRoundRobin;
use crate::{
    app_state::AppState,
    config::{cost_class::CostClass, credentials::ProviderCredentialId},
    dispatcher::DispatcherService,
    endpoints::EndpointType,
    middleware::mapper::model::ModelMapper,
    router::{
        capability::ModelCapability,
        provider_attempt::{ModelCooldownKey, ProviderState},
    },
    types::{provider::InferenceProvider, router::RouterId},
};

#[derive(Debug, Clone)]
pub(crate) struct BudgetCandidate {
    pub credential_id: ProviderCredentialId,
    pub credential_budget_rank: u16,
    pub credential_cost_class: CostClass,
    pub credential_tier: String,
    pub capability: ModelCapability,
    pub service: DispatcherService,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(super) enum CandidateSelectionMode {
    CapabilityThenBudget,
    BudgetThenCapability,
}

#[derive(Debug, Clone)]
pub struct BudgetAwareRouter {
    pub(super) app_state: AppState,
    pub(super) router_id: RouterId,
    pub(super) endpoint_type: EndpointType,
    pub(super) strategy: &'static str,
    pub(super) candidates: Arc<Vec<BudgetCandidate>>,
    pub(super) model_mapper: ModelMapper,
    pub(super) states: Arc<Mutex<HashMap<ProviderCredentialId, ProviderState>>>,
    pub(super) model_states:
        Arc<Mutex<HashMap<ModelCooldownKey, ProviderState>>>,
    pub(super) provider_priorities: Arc<IndexMap<InferenceProvider, u16>>,
    pub(super) default_latency: Duration,
    pub(super) max_cooldown_wait: Duration,
    pub(super) selection_mode: CandidateSelectionMode,
    pub(super) credential_round_robin: Arc<CredentialRoundRobin>,
    pub(super) source_model_selection:
        crate::config::router::SourceModelSelection,
}
