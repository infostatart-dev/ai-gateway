use crate::{app_state::AppState, error::runtime::RuntimeError};
use futures::future::{self, BoxFuture};
use meltdown::Token;
use tokio::time;
use tracing::{Instrument, debug, error};

#[derive(Debug, Clone)]
pub struct HealthMonitor {
    app_state: AppState,
}

impl HealthMonitor {
    #[must_use]
    pub fn new(app_state: AppState) -> Self {
        Self { app_state }
    }

    pub async fn run_forever(self) -> Result<(), RuntimeError> {
        tracing::info!("starting health and uptime monitors");
        let mut interval = time::interval(
            self.app_state.config().discover.monitor.health_interval(),
        );
        loop {
            interval.tick().await;
            let mut monitors = self.app_state.0.health_monitors.write().await;
            let mut check_futures = Vec::new();
            for (router_id, monitor) in monitors.iter_mut() {
                let router_id = router_id.clone();
                let mut monitor = monitor.clone();
                let router_id_for_span = router_id.clone();
                check_futures.push(async move {
                    let result = monitor.check_monitor().await;
                    if let Err(e) = &result { error!(router_id = ?router_id, error = ?e, "Provider health monitor check failed"); }
                    result
                }.instrument(tracing::info_span!("health_monitor", router_id = ?router_id_for_span)));
            }
            if let Err(e) = future::try_join_all(check_futures).await {
                error!(error = ?e, "Provider health monitor encountered an error");
                return Err(e);
            }
        }
    }
}

impl meltdown::Service for HealthMonitor {
    type Future = BoxFuture<'static, Result<(), RuntimeError>>;
    fn run(self, mut token: Token) -> Self::Future {
        Box::pin(async move {
            tokio::select! {
                result = self.run_forever() => {
                    if let Err(e) = result { error!(name = "provider-health-monitor-task", error = ?e, "Monitor encountered error, shutting down"); }
                    else { debug!(name = "provider-health-monitor-task", "Monitor shut down successfully"); }
                    token.trigger();
                }
                () = &mut token => { debug!(name = "provider-health-monitor-task", "task shut down successfully"); }
            }
            Ok(())
        })
    }
}
