use std::sync::Arc;
use tokio::sync::mpsc::Sender;
use tower::discover::Change;
use crate::{
    app_state::AppState,
    config::router::RouterConfig,
    discover::{
        model::{key::Key as ModelKey, weighted_key::WeightedKey as ModelWeightedKey},
        provider::{key::Key as ProviderKey, weighted_key::WeightedKey as ProviderWeightedKey},
    },
    dispatcher::DispatcherService,
    types::router::RouterId,
};
use super::ProviderHealthMonitor;

impl AppState {
    pub async fn add_provider_weighted_router_health_monitor(&self, router_id: RouterId, router_config: Arc<RouterConfig>, tx: Sender<Change<ProviderWeightedKey, DispatcherService>>) {
        self.0.health_monitors.write().await.insert(router_id.clone(), ProviderHealthMonitor::provider_weighted(tx, router_id, router_config, self.clone()));
    }

    pub async fn add_model_weighted_router_health_monitor(&self, router_id: RouterId, router_config: Arc<RouterConfig>, tx: Sender<Change<ModelWeightedKey, DispatcherService>>) {
        self.0.health_monitors.write().await.insert(router_id.clone(), ProviderHealthMonitor::model_weighted(tx, router_id, router_config, self.clone()));
    }

    pub async fn add_provider_latency_router_health_monitor(&self, router_id: RouterId, router_config: Arc<RouterConfig>, tx: Sender<Change<ProviderKey, DispatcherService>>) {
        self.0.health_monitors.write().await.insert(router_id.clone(), ProviderHealthMonitor::provider_latency(tx, router_id, router_config, self.clone()));
    }

    pub async fn add_model_latency_router_health_monitor(&self, router_id: RouterId, router_config: Arc<RouterConfig>, tx: Sender<Change<ModelKey, DispatcherService>>) {
        self.0.health_monitors.write().await.insert(router_id.clone(), ProviderHealthMonitor::model_latency(tx, router_id, router_config, self.clone()));
    }
}
