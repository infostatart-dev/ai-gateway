use std::time::Duration;

use futures::{StreamExt, stream::FuturesUnordered};
use rustc_hash::FxHashSet as HashSet;
use tokio::sync::mpsc::Receiver;
use tower::discover::Change;
use tracing::{debug, error, info, warn};

use super::{
    DEFAULT_WAIT_SECONDS, ProviderMonitorInner, RATE_LIMIT_BUFFER_SECONDS,
};
use crate::{
    discover::provider::key::Key as ProviderKey,
    dispatcher::Dispatcher,
    endpoints::ApiEndpoint,
    error::runtime::RuntimeError,
    types::rate_limit::{ProviderRestore, RateLimitEvent},
};

impl ProviderMonitorInner<ProviderKey> {
    pub(crate) fn create_key_for_endpoint(
        api_endpoint: &ApiEndpoint,
    ) -> ProviderKey {
        ProviderKey::new(api_endpoint.provider(), api_endpoint.endpoint_type())
    }

    pub async fn monitor(
        self,
        mut rx: Receiver<RateLimitEvent>,
    ) -> Result<(), RuntimeError> {
        info!(router_id = ?self.router_id, "starting rate limit monitor for latency strategy LB");
        let mut rate_limited_providers: HashSet<ProviderKey> =
            HashSet::default();
        let mut pending_restores: FuturesUnordered<
            ProviderRestore<ProviderKey>,
        > = FuturesUnordered::new();

        loop {
            tokio::select! {
                Some(event) = rx.recv() => {
                    let key = Self::create_key_for_endpoint(&event.api_endpoint);
                    if rate_limited_providers.contains(&key) {
                        info!(provider = ?event.api_endpoint.provider(), endpoint = ?event.api_endpoint.endpoint_type(), "Provider already rate-limited, ignoring duplicate event");
                    } else {
                        debug!(provider = ?event.api_endpoint.provider(), api_endpoint = ?event.api_endpoint, router_id = ?self.router_id, "Removing rate-limited provider from P2C balancer");
                        if let Err(e) = self.tx.send(Change::Remove(key.clone())).await { error!(error = ?e, "Failed to send remove event for rate-limited provider"); }

                        let duration = Duration::from_secs(event.retry_after_seconds.unwrap_or(DEFAULT_WAIT_SECONDS)) + RATE_LIMIT_BUFFER_SECONDS;
                        let restore = ProviderRestore { key: Some(key.clone()), api_endpoint: event.api_endpoint.clone(), timer: tokio::time::sleep(duration) };
                        pending_restores.push(restore);
                        rate_limited_providers.insert(key);
                        info!(provider = ?event.api_endpoint.provider(), endpoint = ?event.api_endpoint.endpoint_type(), duration_secs = duration.as_secs(), "Scheduled provider re-addition");
                    }
                }
                Some((key, api_endpoint)) = pending_restores.next() => {
                    info!(provider = ?api_endpoint.provider(), endpoint = ?api_endpoint.endpoint_type(), "Re-adding provider to P2C balancer after rate limit expired");
                    let service = Dispatcher::new(self.app_state.clone(), &self.router_id, &self.router_config, api_endpoint.provider()).await
                        .inspect_err(|e| { warn!(error = ?e, provider = ?api_endpoint.provider(), api_endpoint = ?api_endpoint, router_id = ?self.router_id, "Failed to create dispatcher for recovered provider"); })?;
                    self.tx.send(Change::Insert(key.clone(), service)).await.map_err(|e| { error!(error = ?e, router_id = ?self.router_id, "Failed to send insert event for recovered provider"); RuntimeError::ChannelSendFailed })?;
                    rate_limited_providers.remove(&key);
                }
                else => { info!("Rate limit channel closed, shutting down P2C monitor"); break; }
            }
        }
        Ok(())
    }
}
