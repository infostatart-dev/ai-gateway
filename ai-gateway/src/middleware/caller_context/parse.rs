use http::HeaderMap;

use crate::types::extensions::WorkUnitSource;

pub const DEFAULT_AGENT_NAME: &str = "unknown-invoker";

#[must_use]
pub fn parse_agent_name(headers: &HeaderMap) -> String {
    header_value(headers, "x-agent-name")
        .or_else(|| header_value(headers, "helicone-property-agent"))
        .unwrap_or_else(|| DEFAULT_AGENT_NAME.to_string())
}

/// Legacy helper: explicit/session headers only (no synthetic fallback).
#[must_use]
pub fn parse_work_unit_id(headers: &HeaderMap) -> Option<String> {
    header_value(headers, "x-work-unit-id")
        .or_else(|| header_value(headers, "helicone-session-id"))
}

/// Full work-unit ladder for router requests (always non-empty).
#[must_use]
pub fn resolve_work_unit(headers: &HeaderMap) -> (String, WorkUnitSource) {
    if let Some(id) = header_value(headers, "x-work-unit-id") {
        return (id, WorkUnitSource::Explicit);
    }
    if let Some(id) = header_value(headers, "helicone-session-id") {
        return (id, WorkUnitSource::HeliconeSession);
    }
    if let Some(id) = header_value(headers, "x-request-id") {
        return (id, WorkUnitSource::RequestId);
    }
    (uuid::Uuid::new_v4().to_string(), WorkUnitSource::Generated)
}

fn header_value(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}
