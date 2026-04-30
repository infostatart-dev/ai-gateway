use super::{DatabaseListener, types::ServiceState};
use crate::{
    app_state::AppState,
    config::deployment_target::DeploymentTarget,
    error::{init::InitError, runtime::RuntimeError},
};
use futures::future::BoxFuture;
use meltdown::Token;
use sqlx::postgres::PgListener;
use tokio::time::{Duration, MissedTickBehavior, interval};
use tracing::{debug, error, info};

impl DatabaseListener {
    pub async fn new(
        database_url: &str,
        app_state: AppState,
    ) -> Result<Self, InitError> {
        let pg_listener =
            PgListener::connect(database_url).await.map_err(|e| {
                error!(error = %e, "failed to create database listener");
                InitError::DatabaseConnection(e)
            })?;
        let tx = tokio::time::timeout(Duration::from_secs(1), async {
            loop {
                if let Some(tx) = app_state.get_router_tx().await {
                    break tx;
                }
                debug!("router_tx not available, retrying...");
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        })
        .await
        .map_err(|_| InitError::RouterTxNotSet)?;

        let DeploymentTarget::Cloud {
            db_poll_interval,
            listener_reconnect_interval,
        } = app_state.config().deployment_target
        else {
            return Err(InitError::DatabaseListenerOnlyCloud);
        };
        let router_store = app_state
            .0
            .router_store
            .as_ref()
            .ok_or(InitError::StoreNotConfigured("router_store"))?
            .clone();

        Ok(Self {
            app_state,
            pg_listener,
            router_store,
            tx,
            last_router_config_versions: rustc_hash::FxHashMap::default(),
            last_api_key_created_at: rustc_hash::FxHashMap::default(),
            poll_interval: db_poll_interval,
            last_poll_time: None,
            listener_reconnect_interval,
        })
    }

    pub async fn run_service(&mut self) -> Result<(), RuntimeError> {
        info!("performing initial database poll");
        if let Err(e) = self.poll_database().await {
            error!(error = %e, "error during initial database poll");
        }
        self.pg_listener.listen("connected_cloud_gateways").await.map_err(|e| { error!(error = %e, "failed to listen on database notification channel"); InitError::DatabaseConnection(e) })?;

        let mut poll_interval = interval(self.poll_interval);
        poll_interval.set_missed_tick_behavior(MissedTickBehavior::Skip);
        let mut reconnect_interval = interval(self.listener_reconnect_interval);
        reconnect_interval.set_missed_tick_behavior(MissedTickBehavior::Skip);
        let mut state = ServiceState::Idle;

        loop {
            match state {
                ServiceState::Idle => {
                    tokio::select! {
                        biased;
                        notification_result = self.pg_listener.recv() => { match notification_result { Ok(n) => state = ServiceState::HandlingNotification(n), Err(e) => error!(error = %e, "error receiving from listener, continuing") } }
                        _ = poll_interval.tick() => state = ServiceState::PollingDatabase,
                        _ = reconnect_interval.tick() => state = ServiceState::Reconnecting,
                    }
                }
                ServiceState::PollingDatabase => {
                    if let Err(e) = self.poll_database().await {
                        error!(error = %e, "error polling database");
                    }
                    state = ServiceState::Idle;
                }
                ServiceState::HandlingNotification(notification) => {
                    if let Err(e) = self
                        .handle_notification(&notification, self.tx.clone())
                        .await
                    {
                        error!(error = %e, "failed to handle db listener notification, continuing");
                    }
                    state = ServiceState::Idle;
                }
                ServiceState::Reconnecting => {
                    info!("periodic reconnection");
                    let _ = self.pg_listener.unlisten_all().await;
                    if let Err(e) = self
                        .pg_listener
                        .listen("connected_cloud_gateways")
                        .await
                    {
                        error!(error = %e, "failed to listen on channel after reconnection");
                    } else {
                        info!(
                            "successfully reconnected and listening on channel"
                        );
                    }
                    state = ServiceState::Idle;
                }
            }
        }
    }
}

impl meltdown::Service for DatabaseListener {
    type Future = BoxFuture<'static, Result<(), RuntimeError>>;
    fn run(mut self, mut token: Token) -> Self::Future {
        Box::pin(async move {
            tokio::select! {
                biased;
                result = self.run_service() => { if let Err(e) = result { error!(error = %e, "database listener service encountered error, shutting down"); } else { debug!("database listener service shut down successfully"); } token.trigger(); }
                () = &mut token => debug!("database listener service shutdown signal received"),
            }
            Ok(())
        })
    }
}
