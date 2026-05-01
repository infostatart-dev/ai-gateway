use std::sync::Arc;

use rustc_hash::FxHashMap as HashMap;
use tokio::sync::{RwLock, mpsc::Sender};
use tower::discover::Change;

use crate::{
    app_state::AppState,
    config::router::RouterConfig,
    discover::{
        model::{
            key::Key as ModelKey, weighted_key::WeightedKey as ModelWeightedKey,
        },
        provider::{
            key::Key as ProviderKey,
            weighted_key::WeightedKey as ProviderWeightedKey,
        },
    },
    dispatcher::DispatcherService,
    types::router::RouterId,
};

pub mod app_state;
pub mod inner;
pub mod model_latency;
pub mod model_weighted;
pub mod monitor;
pub mod provider_latency;
pub mod provider_weighted;

pub use inner::ProviderMonitorInner;
pub use monitor::HealthMonitor;

pub type HealthMonitorMap =
    Arc<RwLock<HashMap<RouterId, ProviderHealthMonitor>>>;

#[derive(Debug, Clone)]
pub enum ProviderHealthMonitor {
    ProviderWeighted(ProviderMonitorInner<ProviderWeightedKey>),
    ModelWeighted(ProviderMonitorInner<ModelWeightedKey>),
    ProviderLatency(ProviderMonitorInner<ProviderKey>),
    ModelLatency(ProviderMonitorInner<ModelKey>),
}

impl ProviderHealthMonitor {
    #[must_use]
    pub fn provider_weighted(
        tx: Sender<Change<ProviderWeightedKey, DispatcherService>>,
        router_id: RouterId,
        router_config: Arc<RouterConfig>,
        app_state: AppState,
    ) -> Self {
        Self::ProviderWeighted(ProviderMonitorInner::new(
            tx,
            router_id,
            router_config,
            app_state,
        ))
    }

    #[must_use]
    pub fn model_weighted(
        tx: Sender<Change<ModelWeightedKey, DispatcherService>>,
        router_id: RouterId,
        router_config: Arc<RouterConfig>,
        app_state: AppState,
    ) -> Self {
        Self::ModelWeighted(ProviderMonitorInner::new(
            tx,
            router_id,
            router_config,
            app_state,
        ))
    }

    #[must_use]
    pub fn provider_latency(
        tx: Sender<Change<ProviderKey, DispatcherService>>,
        router_id: RouterId,
        router_config: Arc<RouterConfig>,
        app_state: AppState,
    ) -> Self {
        Self::ProviderLatency(ProviderMonitorInner::new(
            tx,
            router_id,
            router_config,
            app_state,
        ))
    }

    #[must_use]
    pub fn model_latency(
        tx: Sender<Change<ModelKey, DispatcherService>>,
        router_id: RouterId,
        router_config: Arc<RouterConfig>,
        app_state: AppState,
    ) -> Self {
        Self::ModelLatency(ProviderMonitorInner::new(
            tx,
            router_id,
            router_config,
            app_state,
        ))
    }

    pub async fn check_monitor(
        &mut self,
    ) -> Result<(), crate::error::runtime::RuntimeError> {
        match self {
            Self::ProviderWeighted(inner) => {
                provider_weighted::check_provider_weighted_monitor(inner).await
            }
            Self::ModelWeighted(inner) => {
                model_weighted::check_model_weighted_monitor(inner).await
            }
            Self::ProviderLatency(inner) => {
                provider_latency::check_provider_latency_monitor(inner).await
            }
            Self::ModelLatency(inner) => {
                model_latency::check_model_latency_monitor(inner).await
            }
        }
    }
}
