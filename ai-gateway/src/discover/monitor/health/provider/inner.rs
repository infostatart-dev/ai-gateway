use std::sync::Arc;
use tokio::sync::mpsc::Sender;
use tower::discover::Change;
use rustc_hash::FxHashSet as HashSet;
use crate::{
    app_state::AppState,
    config::{monitor::GracePeriod, router::RouterConfig},
    dispatcher::DispatcherService,
    error::internal::InternalError,
    types::{provider::InferenceProvider, router::RouterId},
};

#[derive(Debug, Clone)]
pub struct ProviderMonitorInner<K> {
    pub(crate) tx: Sender<Change<K, DispatcherService>>,
    pub(crate) router_id: RouterId,
    pub(crate) router_config: Arc<RouterConfig>,
    pub(crate) app_state: AppState,
    pub(crate) unhealthy_keys: HashSet<K>,
}

impl<K> ProviderMonitorInner<K> {
    pub fn new(tx: Sender<Change<K, DispatcherService>>, router_id: RouterId, router_config: Arc<RouterConfig>, app_state: AppState) -> Self {
        Self { tx, router_id, router_config, app_state, unhealthy_keys: HashSet::default() }
    }

    pub fn check_health(&self, provider: &InferenceProvider) -> Result<bool, InternalError> {
        let provider_endpoints = provider.endpoints();
        let config = self.app_state.config();
        let grace_period = config.discover.monitor.grace_period();
        let mut all_healthy = true;
        for endpoint in provider_endpoints {
            let endpoint_metrics = self.app_state.0.endpoint_metrics.health_metrics(endpoint)?;
            let requests = endpoint_metrics.request_count.total();
            match grace_period { GracePeriod::Requests { min_requests } => { if requests < *min_requests { continue; } } }
            let errors = endpoint_metrics.remote_internal_error_count.total();
            let error_ratio = f64::from(errors) / f64::from(requests);
            if error_ratio > config.discover.monitor.error_threshold() { all_healthy = false; }
        }
        Ok(all_healthy)
    }
}
