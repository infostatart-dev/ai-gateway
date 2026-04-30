use super::inner::ProviderMonitorInner;
use crate::{
    config::balance::BalanceConfigInner,
    discover::provider::key::Key as ProviderKey,
    dispatcher::Dispatcher,
    error::{internal::InternalError, runtime::RuntimeError},
};
use opentelemetry::KeyValue;
use tower::discover::Change;
use tracing::{error, trace};

pub async fn check_provider_latency_monitor(
    inner: &mut ProviderMonitorInner<ProviderKey>,
) -> Result<(), RuntimeError> {
    for (endpoint_type, balance_config) in
        inner.router_config.load_balance.as_ref()
    {
        match balance_config {
            BalanceConfigInner::BalancedLatency { providers } => {
                for provider in providers {
                    let key =
                        ProviderKey::new(provider.clone(), *endpoint_type);
                    let is_healthy = inner.check_health(provider)?;
                    let was_unhealthy = inner.unhealthy_keys.contains(&key);

                    if !is_healthy && !was_unhealthy {
                        trace!(provider = ?provider, endpoint_type = ?endpoint_type, "Provider became unhealthy, removing");
                        if let Err(e) =
                            inner.tx.send(Change::Remove(key.clone())).await
                        {
                            error!(error = ?e, "Failed to send remove event for unhealthy provider");
                        }
                        inner.unhealthy_keys.insert(key);
                    } else if is_healthy && was_unhealthy {
                        trace!(provider = ?provider, endpoint_type = ?endpoint_type, "Provider became healthy, adding back");
                        inner.unhealthy_keys.remove(&key);
                        let service = Dispatcher::new(
                            inner.app_state.clone(),
                            &inner.router_id,
                            &inner.router_config,
                            provider.clone(),
                        )
                        .await?;
                        if let Err(e) =
                            inner.tx.send(Change::Insert(key, service)).await
                        {
                            error!(error = ?e, "Failed to send insert event for healthy provider");
                        }
                    }
                    inner.app_state.0.metrics.provider_health.record(
                        if is_healthy { 1 } else { 0 },
                        &[KeyValue::new("provider", provider.to_string())],
                    );
                }
            }
            _ => return Err(InternalError::Internal.into()),
        }
    }
    Ok(())
}
