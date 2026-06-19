use ai_gateway::{
    middleware::caller_context::{
        DEFAULT_AGENT_NAME, parse_agent_name, parse_work_unit_id,
        resolve_work_unit,
    },
    types::extensions::WorkUnitSource,
};
use http::HeaderMap;

#[test]
fn agent_name_prefers_x_agent_name() {
    let mut headers = HeaderMap::new();
    headers.insert("x-agent-name", "invoker-alpha".parse().unwrap());
    headers.insert("helicone-property-agent", "invoker-beta".parse().unwrap());
    assert_eq!(parse_agent_name(&headers), "invoker-alpha");
}

#[test]
fn agent_name_falls_back_to_helicone_property() {
    let mut headers = HeaderMap::new();
    headers.insert("helicone-property-agent", "invoker-gamma".parse().unwrap());
    assert_eq!(parse_agent_name(&headers), "invoker-gamma");
}

#[test]
fn agent_name_defaults_unknown() {
    assert_eq!(parse_agent_name(&HeaderMap::new()), DEFAULT_AGENT_NAME);
}

#[test]
fn work_unit_prefers_explicit_header() {
    let mut headers = HeaderMap::new();
    headers.insert("x-work-unit-id", "job-47".parse().unwrap());
    headers.insert("helicone-session-id", "sess-abc".parse().unwrap());
    assert_eq!(parse_work_unit_id(&headers).as_deref(), Some("job-47"));
    let (id, source) = resolve_work_unit(&headers);
    assert_eq!(id, "job-47");
    assert_eq!(source, WorkUnitSource::Explicit);
}

#[test]
fn work_unit_falls_back_to_session_id() {
    let mut headers = HeaderMap::new();
    headers.insert("helicone-session-id", "unit-48".parse().unwrap());
    assert_eq!(parse_work_unit_id(&headers).as_deref(), Some("unit-48"));
    let (id, source) = resolve_work_unit(&headers);
    assert_eq!(id, "unit-48");
    assert_eq!(source, WorkUnitSource::HeliconeSession);
}

#[test]
fn work_unit_falls_back_to_request_id() {
    let mut headers = HeaderMap::new();
    headers.insert("x-request-id", "req-99".parse().unwrap());
    assert!(parse_work_unit_id(&headers).is_none());
    let (id, source) = resolve_work_unit(&headers);
    assert_eq!(id, "req-99");
    assert_eq!(source, WorkUnitSource::RequestId);
}

#[test]
fn work_unit_generates_uuid_when_headers_missing() {
    assert!(parse_work_unit_id(&HeaderMap::new()).is_none());
    let (id, source) = resolve_work_unit(&HeaderMap::new());
    assert!(!id.is_empty());
    assert_eq!(source, WorkUnitSource::Generated);
    assert_ne!(resolve_work_unit(&HeaderMap::new()).0, id);
}
