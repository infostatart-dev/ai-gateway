use super::{
    DEFAULT_WAIT_SECONDS, ProviderMonitorInner, RATE_LIMIT_BUFFER_SECONDS,
};
use crate::{
    config::balance::BalanceConfigInner,
    discover::provider::weighted_key::WeightedKey as ProviderWeightedKey,
    dispatcher::Dispatcher,
    endpoints::ApiEndpoint,
    error::{internal::InternalError, runtime::RuntimeError},
    types::rate_limit::{ProviderRestore, RateLimitEvent},
};
use futures::{StreamExt, stream::FuturesUnordered};
use rust_decimal::prelude::ToPrimitive;
use rustc_hash::FxHashMap as HashMap;
use std::time::{Duration, Instant};
use tokio::sync::mpsc::Receiver;
use tower::discover::Change;
use tracing::{debug, error, info};
use weighted_balance::weight::Weight;

impl ProviderMonitorInner<ProviderWeightedKey> {
    fn create_key_for_endpoint(
        &self,
        api_endpoint: &ApiEndpoint,
    ) -> Result<ProviderWeightedKey, InternalError> {
        let provider = api_endpoint.provider();
        let endpoint_type = api_endpoint.endpoint_type();
        let balance_config = self.router_config.load_balance.0.get(&endpoint_type).ok_or_else(|| { error!(router_id = ?self.router_id, endpoint_type = ?endpoint_type, "No balance config found for endpoint type"); InternalError::Internal })?;

        match balance_config {
            BalanceConfigInner::ProviderWeighted { providers } => {
                for target in providers {
                    if target.provider == provider {
                        let weight = Weight::from(
                            target
                                .weight
                                .to_f64()
                                .ok_or(InternalError::Internal)?,
                        );
                        return Ok(ProviderWeightedKey::new(
                            provider,
                            endpoint_type,
                            weight,
                        ));
                    }
                }
                error!(router_id = ?self.router_id, endpoint_type = ?endpoint_type, api_endpoint = ?api_endpoint, "No target found for API endpoint in balance config");
                Err(InternalError::Internal)
            }
            _ => Err(InternalError::Internal),
        }
    }

    pub async fn monitor(
        self,
        mut rx: Receiver<RateLimitEvent>,
    ) -> Result<(), RuntimeError> {
        debug!(router_id = ?self.router_id, "starting rate limit monitor for weighted strategy LB");
        let mut rate_limited_providers: HashMap<ProviderWeightedKey, Instant> =
            HashMap::default();
        let mut pending_restores: FuturesUnordered<
            ProviderRestore<ProviderWeightedKey>,
        > = FuturesUnordered::new();

        loop {
            tokio::select! {
                Some(event) = rx.recv() => {
                    let key = match self.create_key_for_endpoint(&event.api_endpoint) { Ok(k) => k, Err(_) => continue };
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
