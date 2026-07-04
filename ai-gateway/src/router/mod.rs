pub mod budget_aware;
pub mod budget_probe;
pub mod capability;
pub mod direct;
pub mod failover;
pub mod intent;
pub mod latency;
pub mod managed;
pub mod meta;
pub mod pacing;
pub mod provider_attempt;
pub mod quota_admission;
pub mod retry_after;
pub mod routed_identity;
pub mod router_details;
pub mod service;
pub mod strategy;
pub mod token_estimate;
pub mod unified_api;
pub mod upstream_failure;

pub(in crate::router) const FORCED_ROUTING_HEADER: http::HeaderName =
    http::HeaderName::from_static("helicone-forced-routing");
