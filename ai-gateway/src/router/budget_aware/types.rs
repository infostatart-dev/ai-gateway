use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Duration,
};

use indexmap::IndexMap;

use crate::{
    dispatcher::DispatcherService,
    middleware::mapper::model::ModelMapper,
    router::{capability::ModelCapability, provider_attempt::ProviderState},
    types::provider::InferenceProvider,
};

#[derive(Debug, Clone)]
pub(crate) struct BudgetCandidate {
    pub capability: ModelCapability,
    pub service: DispatcherService,
}

#[derive(Debug, Clone)]
pub struct BudgetAwareRouter {
    pub(super) candidates: Arc<Vec<BudgetCandidate>>,
    pub(super) model_mapper: ModelMapper,
    pub(super) states: Arc<Mutex<HashMap<InferenceProvider, ProviderState>>>,
    pub(super) provider_priorities: Arc<IndexMap<InferenceProvider, u16>>,
    pub(super) default_latency: Duration,
    pub(super) max_cooldown_wait: Duration,
}
