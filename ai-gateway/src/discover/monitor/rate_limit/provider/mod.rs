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
use rustc_hash::FxHashMap as HashMap;
use std::{sync::Arc, time::Duration};
use tokio::sync::{RwLock, mpsc::Sender};
use tower::discover::Change;

pub mod app_state;
pub mod model_latency;
pub mod model_weighted;
pub mod monitor;
pub mod provider_latency;
pub mod provider_weighted;
pub use monitor::RateLimitMonitor;

pub const DEFAULT_WAIT_SECONDS: u64 = 30;
#[cfg(not(any(feature = "testing", test)))]
pub const RATE_LIMIT_BUFFER_SECONDS: Duration = Duration::from_secs(30);
#[cfg(any(feature = "testing", test))]
pub const RATE_LIMIT_BUFFER_SECONDS: Duration = Duration::from_secs(1);

#[cfg(not(any(feature = "testing", test)))]
pub const RATE_LIMIT_MONITOR_INTERVAL: Duration = Duration::from_secs(2);
#[cfg(any(feature = "testing", test))]
pub const RATE_LIMIT_MONITOR_INTERVAL: Duration = Duration::from_millis(100);

pub type RateLimitMonitorMap =
    Arc<RwLock<HashMap<RouterId, ProviderRateLimitMonitor>>>;

#[derive(Debug)]
pub enum ProviderRateLimitMonitor {
    ProviderWeighted(ProviderMonitorInner<ProviderWeightedKey>),
    ModelWeighted(ProviderMonitorInner<ModelWeightedKey>),
    ProviderLatency(ProviderMonitorInner<ProviderKey>),
    ModelLatency(ProviderMonitorInner<ModelKey>),
}

#[derive(Debug)]
pub struct ProviderMonitorInner<K> {
    pub(crate) tx: Sender<Change<K, DispatcherService>>,
    pub(crate) router_id: RouterId,
    pub(crate) router_config: Arc<RouterConfig>,
    pub(crate) app_state: AppState,
}

impl<K> ProviderMonitorInner<K> {
    pub fn new(
        tx: Sender<Change<K, DispatcherService>>,
        router_id: RouterId,
        router_config: Arc<RouterConfig>,
        app_state: AppState,
    ) -> Self {
        Self {
            tx,
            router_id,
            router_config,
            app_state,
        }
    }
}

impl ProviderRateLimitMonitor {
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
}
