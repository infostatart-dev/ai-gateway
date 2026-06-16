use std::{collections::HashMap, sync::Arc};

use derive_more::{AsRef, From, Into};

use super::{
    model_id::ModelId, org::OrgId, provider::InferenceProvider,
    router::RouterId, user::UserId,
};
use crate::{config::router::RouterConfig, types::secret::Secret};

#[derive(Debug, Clone, AsRef, From, Into)]
pub struct ProviderRequestId(pub(crate) http::HeaderValue);

/// Winning provider/model after router selection or failover.
#[derive(Debug, Clone)]
pub struct RoutedModelAndProvider(pub String);

#[derive(Debug, Clone)]
pub struct AuthContext {
    pub api_key: Secret<String>,
    pub user_id: UserId,
    pub org_id: OrgId,
}

#[derive(Debug)]
pub struct RequestContext {
    /// If `None`, the request was for a direct proxy.
    /// If `Some`, the request was for a load balanced router.
    pub router_config: Option<Arc<RouterConfig>>,
    /// If `None`, the router is configured to not require auth for requests,
    /// disabling some features.
    pub auth_context: Option<AuthContext>,
}

#[derive(Debug, Clone)]
pub struct MapperContext {
    pub is_stream: bool,
    /// If `None`, the request was for an endpoint without
    /// first class support for mapping between different provider
    /// models.
    pub model: Option<ModelId>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct PromptContext {
    pub prompt_id: String,
    pub prompt_version_id: Option<String>,
    pub inputs: Option<HashMap<String, serde_json::Value>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequestKind {
    Router,
    UnifiedApi,
    DirectProxy,
}

/// Per-request routing labels for router runtime OTEL metrics (`router_*`).
#[derive(Debug, Clone)]
pub struct RouterRuntimeLabels {
    pub router_id: RouterId,
    pub endpoint_type: String,
    pub strategy: &'static str,
}

/// Upstream hop identity for provider observability (`gateway_provider_*`).
#[derive(Debug, Clone)]
pub struct UpstreamAttemptContext {
    pub attempt_index: u32,
    pub upstream_attempts: u32,
    pub credential: String,
}

/// Terminal JSON header payload (`X-Gateway-Provider-Usage`).
#[derive(Debug, Clone)]
pub struct GatewayProviderUsageExtension(
    pub crate::metrics::provider::GatewayProviderUsage,
);

/// Deferred routing trace emission after response body metrics are collected.
#[derive(Debug, Clone)]
pub struct PendingRouteTrace {
    pub router_id: RouterId,
    pub strategy: &'static str,
    pub hops: u32,
    pub candidates: usize,
    pub skipped: usize,
    pub outcome_label: &'static str,
    pub terminal_provider: Option<InferenceProvider>,
    pub terminal_credential: Option<String>,
    pub terminal_status: Option<u16>,
    pub deepseek_web: Option<crate::router::budget_aware::DeepSeekWebTrace>,
}
