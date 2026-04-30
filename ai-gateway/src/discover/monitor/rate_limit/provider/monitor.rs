use futures::future::BoxFuture;
use meltdown::Token;
use tokio::{task::JoinSet, time};
use tracing::{debug, error, info};
use crate::{
    app_state::AppState,
    error::runtime::RuntimeError,
};
use super::{ProviderRateLimitMonitor, RATE_LIMIT_MONITOR_INTERVAL};

#[derive(Debug)]
pub struct RateLimitMonitor {
    app_state: AppState,
    tasks: JoinSet<Result<(), RuntimeError>>,
}

impl RateLimitMonitor {
    #[must_use]
    pub fn new(app_state: AppState) -> Self {
        Self { app_state, tasks: JoinSet::new() }
    }

    pub async fn run_forever(mut self) -> Result<(), RuntimeError> {
        debug!("Starting provider rate limit monitors");
        let mut interval = time::interval(RATE_LIMIT_MONITOR_INTERVAL);
        let app_state = self.app_state.clone();

        loop {
            tokio::select! {
                Some(res) = self.tasks.join_next() => {
                    match res {
                        Ok(Ok(())) => info!("Rate limit monitor task shutdown successfully"),
                        Ok(Err(e)) => { error!(error = ?e, "Rate limit monitor task failed"); return Err(e); },
                        Err(e) => { error!(error = ?e, "Tokio runtime failed to join rate limit monitor task"); return Err(e.into()); },
                    }
                }
                _ = interval.tick() => {
                    let mut monitors = app_state.0.rate_limit_monitors.write().await;
                    for (router_id, monitor) in monitors.drain() {
                        let rx = app_state.remove_rate_limit_receiver(&router_id).await?;
                        match monitor {
                            ProviderRateLimitMonitor::ProviderWeighted(inner) => { self.tasks.spawn(inner.monitor(rx)); },
                            ProviderRateLimitMonitor::ModelWeighted(inner) => { self.tasks.spawn(inner.monitor(rx)); },
                            ProviderRateLimitMonitor::ProviderLatency(inner) => { self.tasks.spawn(inner.monitor(rx)); },
                            ProviderRateLimitMonitor::ModelLatency(inner) => { self.tasks.spawn(inner.monitor(rx)); },
                        }
                    }
                }
            }
        }
    }
}

impl meltdown::Service for RateLimitMonitor {
    type Future = BoxFuture<'static, Result<(), RuntimeError>>;

    fn run(self, mut token: Token) -> Self::Future {
        Box::pin(async move {
            tokio::select! {
                result = self.run_forever() => {
                    if let Err(e) = result { error!(name = "provider-rate-limit-monitor-task", error = ?e, "Monitor encountered error, shutting down"); }
                    else { debug!(name = "provider-rate-limit-monitor-task", "Monitor shut down successfully"); }
                    token.trigger();
                }
                () = &mut token => { debug!(name = "provider-rate-limit-monitor-task", "task shut down successfully"); }
            }
            Ok(())
        })
    }
}
