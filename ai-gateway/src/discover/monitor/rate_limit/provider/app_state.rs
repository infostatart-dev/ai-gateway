use super::ProviderRateLimitMonitor;
use crate::{
    app_state::AppState,
    config::router::RouterConfig,
    discover::{
        model::key::Key as ModelKey,
        model::weighted_key::WeightedKey as ModelWeightedKey,
        provider::key::Key as ProviderKey,
        provider::weighted_key::WeightedKey as ProviderWeightedKey,
    },
    dispatcher::DispatcherService,
    error::init::InitError,
    types::{rate_limit::RateLimitEvent, router::RouterId},
};
use std::sync::Arc;
use tokio::sync::mpsc::{Receiver, Sender};
use tower::discover::Change;
use tracing::warn;

impl AppState {
    pub async fn add_provider_weighted_router_rate_limit_monitor(
        &self,
        router_id: RouterId,
        router_config: Arc<RouterConfig>,
        tx: Sender<Change<ProviderWeightedKey, DispatcherService>>,
    ) {
        self.0.rate_limit_monitors.write().await.insert(
            router_id.clone(),
            ProviderRateLimitMonitor::provider_weighted(
                tx,
                router_id,
                router_config,
                self.clone(),
            ),
        );
    }

    pub async fn add_model_weighted_router_rate_limit_monitor(
        &self,
        router_id: RouterId,
        router_config: Arc<RouterConfig>,
        tx: Sender<Change<ModelWeightedKey, DispatcherService>>,
    ) {
        self.0.rate_limit_monitors.write().await.insert(
            router_id.clone(),
            ProviderRateLimitMonitor::model_weighted(
                tx,
                router_id,
                router_config,
                self.clone(),
            ),
        );
    }

    pub async fn add_provider_latency_router_rate_limit_monitor(
        &self,
        router_id: RouterId,
        router_config: Arc<RouterConfig>,
        tx: Sender<Change<ProviderKey, DispatcherService>>,
    ) {
        self.0.rate_limit_monitors.write().await.insert(
            router_id.clone(),
            ProviderRateLimitMonitor::provider_latency(
                tx,
                router_id,
                router_config,
                self.clone(),
            ),
        );
    }

    pub async fn add_model_latency_router_rate_limit_monitor(
        &self,
        router_id: RouterId,
        router_config: Arc<RouterConfig>,
        tx: Sender<Change<ModelKey, DispatcherService>>,
    ) {
        self.0.rate_limit_monitors.write().await.insert(
            router_id.clone(),
            ProviderRateLimitMonitor::model_latency(
                tx,
                router_id,
                router_config,
                self.clone(),
            ),
        );
    }

    pub async fn remove_rate_limit_receiver(
        &self,
        router_id: &RouterId,
    ) -> Result<Receiver<RateLimitEvent>, InitError> {
        let Some(rx) =
            self.0.rate_limit_receivers.write().await.remove(router_id)
        else {
            warn!(router_id = ?router_id, "No rate limit receiver found for router");
            return Err(InitError::RateLimitChannelsNotInitialized(
                router_id.clone(),
            ));
        };
        Ok(rx)
    }
}
