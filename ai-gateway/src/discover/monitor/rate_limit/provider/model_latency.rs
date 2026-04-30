use std::time::{Duration, Instant};
use futures::{StreamExt, stream::FuturesUnordered};
use tokio::sync::mpsc::Receiver;
use tower::discover::Change;
use tracing::{debug, error, info};
use rustc_hash::FxHashMap as HashMap;
use crate::{
    discover::model::key::Key as ModelKey,
    dispatcher::Dispatcher,
    error::{internal::InternalError, runtime::RuntimeError},
    types::rate_limit::{ProviderRestore, RateLimitEvent},
};
use super::{ProviderMonitorInner, DEFAULT_WAIT_SECONDS, RATE_LIMIT_BUFFER_SECONDS};

impl ProviderMonitorInner<ModelKey> {
    fn create_model_latency_key(&self, event: &RateLimitEvent) -> Result<ModelKey, InternalError> {
        let model_id = event.model_id.clone().ok_or_else(|| { error!(router_id = ?self.router_id, api_endpoint = ?event.api_endpoint, "No model id found in rate limit event"); InternalError::Internal })?;
        Ok(ModelKey::new(model_id, event.api_endpoint.endpoint_type()))
    }

    pub async fn monitor(self, mut rx: Receiver<RateLimitEvent>) -> Result<(), RuntimeError> {
        debug!(router_id = ?self.router_id, "starting rate limit monitor for weighted strategy LB");
        let mut rate_limited_providers: HashMap<ModelKey, Instant> = HashMap::default();
        let mut pending_restores: FuturesUnordered<ProviderRestore<ModelKey>> = FuturesUnordered::new();

        loop {
            tokio::select! {
                Some(event) = rx.recv() => {
                    let key = match self.create_model_latency_key(&event) { Ok(k) => k, Err(_) => continue };
                    if let std::collections::hash_map::Entry::Vacant(e) = rate_limited_providers.entry(key.clone()) {
                        debug!(provider = ?event.api_endpoint.provider(), api_endpoint = ?event.api_endpoint, router_id = ?self.router_id, "Removing rate-limited provider from Weighted balancer");
                        if let Err(e) = self.tx.send(Change::Remove(key.clone())).await { error!(error = ?e, "Failed to send remove event for rate-limited provider"); }
                        e.insert(Instant::now());
                        let duration = Duration::from_secs(event.retry_after_seconds.unwrap_or(DEFAULT_WAIT_SECONDS)) + RATE_LIMIT_BUFFER_SECONDS;
                        info!(provider = ?event.api_endpoint.provider(), endpoint_type = ?event.api_endpoint.endpoint_type(), api_endpoint = ?event.api_endpoint, router_id = ?self.router_id, duration_secs = duration.as_secs(), "Scheduled provider re-addition");
                        pending_restores.push(ProviderRestore { key: Some(key), api_endpoint: event.api_endpoint.clone(), timer: tokio::time::sleep(duration) });
                    } else {
                        info!(provider = ?event.api_endpoint.provider(), endpoint = ?event.api_endpoint.endpoint_type(), "Provider already rate-limited, ignoring duplicate event");
                    }
                }
                Some((key, api_endpoint)) = pending_restores.next() => {
                    info!(provider = ?api_endpoint.provider(), endpoint = ?api_endpoint.endpoint_type(), api_endpoint = ?api_endpoint, router_id = ?self.router_id, "Re-adding provider to Weighted balancer after rate limit expired");
                    let service = Dispatcher::new(self.app_state.clone(), &self.router_id, &self.router_config, api_endpoint.provider()).await
                        .inspect_err(|e| { error!(error = ?e, provider = ?api_endpoint.provider(), api_endpoint = ?api_endpoint, router_id = ?self.router_id, "Failed to create dispatcher for recovered provider"); })?;
                    self.tx.send(Change::Insert(key.clone(), service)).await.map_err(|e| { error!(error = ?e, router_id = ?self.router_id, "Failed to send insert event for recovered provider"); RuntimeError::ChannelSendFailed })?;
                    rate_limited_providers.remove(&key);
                }
                else => { info!("Rate limit channel closed, shutting down Weighted monitor"); break; }
            }
        }
        Ok(())
    }
}
