use chrono::Utc;
use tracing::{debug, error, info};

use super::DatabaseListener;
use crate::{
    config::router::RouterConfig,
    control_plane::types::Key,
    error::runtime::RuntimeError,
    types::{org::OrgId, router::RouterId, user::UserId},
};

impl DatabaseListener {
    #[allow(clippy::too_many_lines)]
    pub async fn poll_database(&mut self) -> Result<(), RuntimeError> {
        let start = Utc::now();
        info!("polling database for changes");

        let new_api_keys = if let Some(last_poll) = self.last_poll_time {
            self.router_store
                .get_all_db_helicone_api_keys_updated_after(last_poll)
                .await
                .inspect(|keys| {
                    debug!(
                        "polling found {} new helicone api keys",
                        keys.len()
                    );
                })
                .inspect_err(|e| {
                    error!(error = %e, "failed to poll api keys");
                })?
        } else {
            Vec::new()
        };

        for api_key in new_api_keys {
            let soft_delete = api_key.soft_delete.unwrap_or(false);
            let key_timestamp =
                api_key.updated_at.unwrap_or(api_key.created_at);
            let should_process =
                match self.last_api_key_created_at.get(&api_key.key_hash) {
                    None => true,
                    Some(last_seen) => key_timestamp > *last_seen,
                };

            if should_process {
                if soft_delete {
                    self.app_state
                        .remove_helicone_api_key(api_key.key_hash.clone())
                        .await?;
                    self.last_api_key_created_at.remove(&api_key.key_hash);
                } else {
                    self.app_state
                        .set_helicone_api_key(Key {
                            key_hash: api_key.key_hash.clone(),
                            owner_id: UserId::new(api_key.owner_id),
                            organization_id: OrgId::new(
                                api_key.organization_id,
                            ),
                        })
                        .await?;
                    self.last_api_key_created_at
                        .insert(api_key.key_hash, key_timestamp);
                }
            }
        }

        let new_routers = if let Some(last_poll) = self.last_poll_time {
            self.router_store
                .get_routers_created_after(last_poll)
                .await
                .inspect(|routers| {
                    debug!("polling found {} new routers", routers.len());
                })
                .inspect_err(|e| {
                    error!(error = %e, "failed to poll router configs");
                })?
        } else {
            self.router_store
                .get_all_routers()
                .await
                .inspect(|routers| {
                    info!("polling initialized with {} routers", routers.len());
                })
                .inspect_err(|e| {
                    error!(error = %e, "failed to poll router configs");
                })?
        };

        for db_router in new_routers {
            let should_process = match self
                .last_router_config_versions
                .get(&db_router.router_hash)
            {
                None => true,
                Some(last_seen) => db_router.created_at > *last_seen,
            };
            if should_process {
                match serde_json::from_value::<RouterConfig>(db_router.config) {
                    Ok(config) => {
                        info!(router_hash = %db_router.router_hash, "polling found new/updated router");
                        self.handle_router_config_insert(
                            RouterId::Named(
                                db_router.router_hash.clone().into(),
                            ),
                            config,
                            OrgId::new(db_router.organization_id),
                            self.tx.clone(),
                        )
                        .await?;
                        self.last_router_config_versions.insert(
                            db_router.router_hash,
                            db_router.created_at,
                        );
                    }
                    Err(e) => {
                        error!(error = %e, router_hash = %db_router.router_hash, "failed to parse router config");
                    }
                }
            }
        }

        let end = Utc::now();
        self.last_poll_time = Some(end);
        info!(
            poll_duration_ms = (end - start).num_milliseconds(),
            "database polling complete"
        );
        Ok(())
    }
}
