use std::sync::Arc;
use tokio::sync::mpsc::Sender;
use tower::discover::Change;
use tracing::error;
use crate::{
    config::router::RouterConfig,
    error::{internal::InternalError, runtime::RuntimeError},
    router::service::Router,
    types::{org::OrgId, router::RouterId},
};
use super::DatabaseListener;

impl DatabaseListener {
    pub async fn handle_router_config_insert(&self, router_hash: RouterId, router_config: RouterConfig, organization_id: OrgId, tx: Sender<Change<RouterId, Router>>) -> Result<(), RuntimeError> {
        let router = Router::new(router_hash.clone(), Arc::new(router_config), self.app_state.clone()).await?;
        tx.send(Change::Insert(router_hash.clone(), router)).await.map_err(|e| { error!(error = %e, "failed to send router insert to tx"); RuntimeError::Internal(InternalError::Internal) })?;
        self.app_state.set_router_organization(router_hash, organization_id).await;
        let provider_keys = self.router_store.get_org_provider_keys(organization_id).await?;
        self.app_state.0.provider_keys.set_org_provider_keys(organization_id, provider_keys).await;
        Ok(())
    }
}
