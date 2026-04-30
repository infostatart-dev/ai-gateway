use super::{
    DatabaseListener,
    types::{ConnectedCloudGatewaysNotification, Op},
};
use crate::{
    control_plane::types::Key,
    error::{internal::InternalError, runtime::RuntimeError},
    router::service::Router,
    types::router::RouterId,
};
use chrono::Utc;
use tokio::sync::mpsc::Sender;
use tower::discover::Change;
use tracing::{debug, error, info};

impl DatabaseListener {
    #[allow(clippy::too_many_lines)]
    pub async fn handle_notification(
        &mut self,
        notification: &sqlx::postgres::PgNotification,
        tx: Sender<Change<RouterId, Router>>,
    ) -> Result<(), RuntimeError> {
        info!(channel = notification.channel(), "processing notification");
        if notification.channel() == "connected_cloud_gateways" {
            let payload = serde_json::from_str::<ConnectedCloudGatewaysNotification>(notification.payload()).map_err(|e| { error!(error = %e, "failed to parse connected_cloud_gateways payload"); InternalError::Deserialize { ty: "ConnectedCloudGatewaysNotification", error: e } })?;
            match payload {
                ConnectedCloudGatewaysNotification::RouterConfigUpdated {
                    router_hash,
                    organization_id,
                    op,
                    config,
                    ..
                } => {
                    info!(router_hash = %router_hash, organization_id = %organization_id, "router configuration created/updated");
                    match op {
                        Op::Insert => {
                            self.app_state.increment_router_metrics(
                                &router_hash,
                                &config,
                                Some(organization_id),
                            );
                            self.handle_router_config_insert(router_hash.clone(), *config, organization_id, tx).await.map_err(|e| { error!(error = %e, "failed to handle router config insert"); e })?;
                            self.last_router_config_versions
                                .insert(router_hash.to_string(), Utc::now());
                            Ok(())
                        }
                        Op::Delete => {
                            self.app_state.decrement_router_metrics(
                                &router_hash,
                                &config,
                                Some(organization_id),
                            );
                            tx.send(Change::Remove(router_hash.clone())).await.map_err(|e| { error!(error = %e, "failed to send router remove to tx"); RuntimeError::Internal(InternalError::Internal) })?;
                            info!(router_hash = %router_hash, organization_id = %organization_id, "router removed");
                            self.last_router_config_versions
                                .remove(&router_hash.to_string());
                            Ok(())
                        }
                        _ => {
                            debug!("skipping router insert");
                            Ok(())
                        }
                    }
                }
                ConnectedCloudGatewaysNotification::ApiKeyUpdated {
                    owner_id,
                    organization_id,
                    api_key_hash,
                    soft_delete,
                    op,
                } => match op {
                    Op::Insert => {
                        self.app_state.set_helicone_api_key(Key { key_hash: api_key_hash.clone(), owner_id, organization_id }).await.map_err(|e| { error!(error = %e, "failed to set helicone api key"); e })?;
                        info!(owner_id = %owner_id, organization_id = %organization_id, "helicone api key inserted");
                        self.last_api_key_created_at
                            .insert(api_key_hash, Utc::now());
                        Ok(())
                    }
                    Op::Delete => {
                        self.app_state.remove_helicone_api_key(api_key_hash.clone()).await.map_err(|e| { error!(error = %e, "failed to remove helicone api key"); e })?;
                        info!(owner_id = %owner_id, organization_id = %organization_id, "helicone api key removed");
                        self.last_api_key_created_at.remove(&api_key_hash);
                        Ok(())
                    }
                    Op::Update => {
                        if soft_delete {
                            self.app_state.remove_helicone_api_key(api_key_hash.clone()).await.map_err(|e| { error!(error = %e, "failed to remove helicone api key"); e })?;
                            info!(owner_id = %owner_id, organization_id = %organization_id, "helicone api key soft deleted");
                            self.last_api_key_created_at.remove(&api_key_hash);
                        } else {
                            self.last_api_key_created_at
                                .insert(api_key_hash, Utc::now());
                        }
                        Ok(())
                    }
                    Op::Truncate => {
                        debug!("skipping helicone api key truncate");
                        Ok(())
                    }
                },
                ConnectedCloudGatewaysNotification::Unknown { data } => {
                    debug!("Unknown notification event: {:?}", data);
                    Ok(())
                }
            }
        } else {
            debug!("received unknown notification");
            Ok(())
        }
    }
}
