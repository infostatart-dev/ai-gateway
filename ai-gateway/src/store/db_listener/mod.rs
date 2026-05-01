use chrono::{DateTime, Utc};
use rustc_hash::FxHashMap as HashMap;
use sqlx::postgres::PgListener;
use tokio::{sync::mpsc::Sender, time::Duration};
use tower::discover::Change;

use crate::{
    app_state::AppState, router::service::Router, store::router::RouterStore,
    types::router::RouterId,
};

pub mod notification;
pub mod poll;
pub mod router;
pub mod service;
pub mod types;

#[derive(Debug)]
pub struct DatabaseListener {
    pub(crate) app_state: AppState,
    pub(crate) pg_listener: PgListener,
    pub(crate) router_store: RouterStore,
    pub(crate) tx: Sender<Change<RouterId, Router>>,
    pub(crate) last_router_config_versions: HashMap<String, DateTime<Utc>>,
    pub(crate) last_api_key_created_at: HashMap<String, DateTime<Utc>>,
    pub(crate) poll_interval: Duration,
    pub(crate) last_poll_time: Option<DateTime<Utc>>,
    pub(crate) listener_reconnect_interval: Duration,
}
