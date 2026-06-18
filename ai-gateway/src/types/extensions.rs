use std::{collections::HashMap, sync::Arc};

use derive_more::{AsRef, From, Into};

use super::{
    model_id::ModelId, org::OrgId, provider::InferenceProvider,
    router::RouterId, user::UserId,
};
use crate::{config::router::RouterConfig, types::secret::Secret};

#[derive(Debug, Clone, AsRef, From, Into)]
pub struct ProviderRequestId(pub(crate) http::HeaderValue);

/// Resolved routing intent attached to successful autodefault responses.
#[derive(Debug, Clone, Copy)]
pub struct RoutingIntentContext {
    pub intent_tier: crate::router::intent::IntentTier,
    pub selection_phase: crate::router::intent::SelectionPhase,
}

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

/// Estimated payload tokens attached by the budget-aware router for pacing.
#[derive(Debug, Clone, Copy)]
pub struct GatewayPayloadEstimate(pub u32);

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

/// Normalized upstream failure attached by provider dispatchers for router
/// policy.
#[derive(Debug, Clone)]
pub struct UpstreamFailureContext {
    pub kind: crate::router::upstream_failure::UpstreamFailureKind,
    pub restricted_until: Option<chrono::DateTime<chrono::Utc>>,
}

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
    pub chatgpt_web: Option<crate::router::budget_aware::ChatGptWebTrace>,
    pub intent_tier: Option<crate::router::intent::IntentTier>,
    pub selection_phase: Option<crate::router::intent::SelectionPhase>,
    pub quota_scope: Option<String>,
    pub model_ladder_band: Option<String>,
    pub model_ladder_position: Option<u16>,
    pub upstream_failure_kind: Option<String>,
    pub restricted_until: Option<String>,
    pub failover_class: Option<String>,
}
