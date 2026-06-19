use http::{HeaderName, HeaderValue, request::Parts};

use crate::{
    middleware::caller_context::{parse_agent_name, resolve_work_unit},
    tests::routing::request_parts,
    types::extensions::CallerRequestContext,
};

#[must_use]
pub fn work_unit_header(id: &str) -> (HeaderName, HeaderValue) {
    (
        HeaderName::from_static("x-work-unit-id"),
        HeaderValue::from_str(id).expect("valid work unit id"),
    )
}

#[must_use]
pub fn agent_header(name: &str) -> (HeaderName, HeaderValue) {
    (
        HeaderName::from_static("x-agent-name"),
        HeaderValue::from_str(name).expect("valid agent name"),
    )
}

#[must_use]
pub fn request_id_header(id: &str) -> (HeaderName, HeaderValue) {
    (
        HeaderName::from_static("x-request-id"),
        HeaderValue::from_str(id).expect("valid request id"),
    )
}

#[must_use]
pub fn caller_parts_with_request_id(agent: &str, request_id: &str) -> Parts {
    let mut parts = request_parts();
    let (agent_key, agent_val) = agent_header(agent);
    parts.headers.insert(agent_key, agent_val);
    let (req_key, req_val) = request_id_header(request_id);
    parts.headers.insert(req_key, req_val);
    let (work_unit_id, work_unit_source) = resolve_work_unit(&parts.headers);
    parts.extensions.insert(CallerRequestContext {
        agent_name: parse_agent_name(&parts.headers),
        work_unit_id: Some(work_unit_id),
        work_unit_source,
    });
    parts
}

#[must_use]
pub fn caller_parts(agent: &str, work_unit: Option<&str>) -> Parts {
    let mut parts = request_parts();
    let (agent_key, agent_val) = agent_header(agent);
    parts.headers.insert(agent_key, agent_val);
    if let Some(id) = work_unit {
        let (unit_key, unit_val) = work_unit_header(id);
        parts.headers.insert(unit_key, unit_val);
    }
    let (work_unit_id, work_unit_source) = resolve_work_unit(&parts.headers);
    parts.extensions.insert(CallerRequestContext {
        agent_name: parse_agent_name(&parts.headers),
        work_unit_id: Some(work_unit_id),
        work_unit_source,
    });
    parts
}
