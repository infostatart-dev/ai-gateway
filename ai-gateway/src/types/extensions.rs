use std::{collections::HashMap, sync::Arc};

use derive_more::{AsRef, From, Into};

use super::{
    model_id::ModelId, org::OrgId, provider::InferenceProvider,
    router::RouterId, user::UserId,
};
use crate::{config::router::RouterConfig, types::secret::Secret};

#[derive(Debug, Clone, AsRef, From, Into)]
pub struct ProviderRequestId(pub(crate) http::HeaderValue);

/// How the gateway resolved `work_unit_id` for a router request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum WorkUnitSource {
    Explicit,
    HeliconeSession,
    RequestId,
    Generated,
}

/// Inbound invoker identity for route planning and observability.
#[derive(Debug, Clone)]
pub struct CallerRequestContext {
    pub agent_name: String,
    pub work_unit_id: Option<String>,
    pub work_unit_source: WorkUnitSource,
}

impl CallerRequestContext {
    #[must_use]
    pub fn work_unit_id_str(&self) -> Option<&str> {
        self.work_unit_id.as_deref().filter(|id| !id.is_empty())
    }
}

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
    pub agent_name: Option<String>,
    pub work_unit_id: Option<String>,
    pub work_unit_source: Option<WorkUnitSource>,
    pub planned_hops: Option<u32>,
    pub plan_rebuilds: Option<u32>,
    pub route_memory_hit: Option<bool>,
    pub route_memory_invalidated: Option<bool>,
    pub source_model: Option<String>,
    pub json_schema_required: bool,
    pub replay: Option<PlanReplaySnapshot>,
}

/// Inputs for route-chain replanning during failover.
#[derive(Debug, Clone)]
pub struct RoutePlanContext {
    pub caller: CallerRequestContext,
    pub full_pool: Vec<crate::router::budget_aware::BudgetCandidate>,
    pub estimated_tokens: u32,
    pub route_memory_hit: bool,
    pub planned_hops: u32,
    pub source_model: Option<String>,
    pub json_schema_required: bool,
    pub replay: Option<PlanReplaySnapshot>,
}

/// Planner-time snapshot for hop-0 replay (D19).
#[derive(Debug, Clone)]
pub struct PlanReplaySnapshot {
    pub plan_snapshot_ts: String,
    pub winner_credential: String,
    pub winner_model: String,
    pub winner: ReplayScoreBreakdown,
    pub top_alternatives: Vec<ReplayAlternative>,
}

/// Deterministic replay payload for post-mortem route analysis (D19).
#[derive(Debug, Clone, serde::Serialize)]
pub struct ReplayRecord {
    pub agent_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub work_unit_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_model: Option<String>,
    pub json_schema_required: bool,
    pub planned_hops: u32,
    pub plan_rebuilds: u32,
    pub route_memory_hit: bool,
    pub route_memory_invalidated: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plan_snapshot_ts: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub winner_credential: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub winner_model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub winner_score: Option<ReplayScoreBreakdown>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub top_alternatives: Vec<ReplayAlternative>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ReplayScoreBreakdown {
    pub score: f64,
    pub h_success: f64,
    #[serde(alias = "q_headroom")]
    pub quota_capacity: f64,
    pub q_cooldown_secs: f64,
    pub m_affinity: f64,
    pub hash_bias: f64,
    pub l_band: u16,
    pub cost_class: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ReplayAlternative {
    pub credential: String,
    pub model: String,
    pub score: f64,
}
