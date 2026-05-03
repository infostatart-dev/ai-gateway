use std::collections::HashSet;

use crate::{
    config::Config, control_plane::types::Key, error::init::InitError,
    metrics::Metrics, store::router::RouterStore,
};

pub(super) async fn load_initial_helicone_api_keys(
    config: &Config,
    router_store: Option<&RouterStore>,
    metrics: &Metrics,
) -> Result<Option<HashSet<Key>>, InitError> {
    if !config.deployment_target.is_cloud() {
        return Ok(None);
    }
    let Some(store) = router_store else {
        return Ok(None);
    };

    let keys = store
        .get_all_helicone_api_keys()
        .await
        .map_err(|e| InitError::InitHeliconeKeys(e.to_string()))?;
    tracing::info!("loaded initial {} helicone api keys", keys.len());
    metrics
        .routers
        .helicone_api_keys
        .add(i64::try_from(keys.len()).unwrap_or(i64::MAX), &[]);
    Ok(Some(keys))
}
