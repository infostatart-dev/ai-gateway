use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Duration,
};

use indexmap::IndexMap;

use crate::{
    app_state::AppState,
    dispatcher::DispatcherService,
    endpoints::EndpointType,
    middleware::mapper::model::ModelMapper,
    router::{capability::ModelCapability, provider_attempt::ProviderState},
    types::{provider::InferenceProvider, router::RouterId},
};

#[derive(Debug, Clone)]
pub(crate) struct BudgetCandidate {
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
    pub(super) states: Arc<Mutex<HashMap<InferenceProvider, ProviderState>>>,
    pub(super) provider_priorities: Arc<IndexMap<InferenceProvider, u16>>,
    pub(super) default_latency: Duration,
    pub(super) max_cooldown_wait: Duration,
    pub(super) selection_mode: CandidateSelectionMode,
}
