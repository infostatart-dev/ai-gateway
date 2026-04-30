use crate::{
    config::router::RouterConfig,
    types::{org::OrgId, router::RouterId, user::UserId},
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, PartialEq)]
pub enum Op {
    #[serde(rename = "INSERT")]
    Insert,
    #[serde(rename = "UPDATE")]
    Update,
    #[serde(rename = "DELETE")]
    Delete,
    #[serde(rename = "TRUNCATE")]
    Truncate,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum ConnectedCloudGatewaysNotification {
    RouterConfigUpdated {
        router_id: String,
        router_hash: RouterId,
        router_config_id: String,
        organization_id: OrgId,
        version: String,
        op: Op,
        config: Box<RouterConfig>,
    },
    ApiKeyUpdated {
        owner_id: UserId,
        organization_id: OrgId,
        api_key_hash: String,
        soft_delete: bool,
        op: Op,
    },
    Unknown {
        #[serde(flatten)]
        data: serde_json::Value,
    },
}

pub enum ServiceState {
    Idle,
    PollingDatabase,
    Reconnecting,
    HandlingNotification(sqlx::postgres::PgNotification),
}
