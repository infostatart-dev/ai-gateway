use bytes::Bytes;

use super::{PayloadBudgetConfig, estimate_from_value};

fn cfg() -> PayloadBudgetConfig {
    PayloadBudgetConfig::default()
}

fn parse(body: &Bytes) -> serde_json::Value {
    serde_json::from_slice(body).expect("test body json")
}

#[test]
fn estimates_input_and_default_output_reservation() {
    let body = Bytes::from(
        r#"{"model":"gpt-5-mini","messages":[{"role":"user","content":"hello world"}]}"#,
    );
    let estimate =
        estimate_from_value(&parse(&body), cfg()).expect("estimate");
    assert!(estimate.input_tokens > 0);
    assert_eq!(estimate.reserved_output, 4_000);
    assert_eq!(estimate.total(), estimate.input_tokens + 4_000);
}

#[test]
fn reserves_explicit_max_tokens() {
    let body = Bytes::from(
        r#"{"max_tokens":256,"messages":[{"role":"user","content":"hi"}]}"#,
    );
    let estimate =
        estimate_from_value(&parse(&body), cfg()).expect("estimate");
    assert_eq!(estimate.reserved_output, 256);
}

#[test]
fn json_schema_increases_estimate() {
    let plain = Bytes::from(
        r#"{"messages":[{"role":"user","content":"classify this ticket"}]}"#,
    );
    let with_schema = Bytes::from(
        r#"{"messages":[{"role":"user","content":"classify this ticket"}],"response_format":{"type":"json_schema","json_schema":{"name":"out","schema":{"type":"object","properties":{"status":{"type":"string"},"priority":{"type":"string"},"summary":{"type":"string"}},"required":["status","priority","summary"],"additionalProperties":false}}}}"#,
    );
    let plain_tokens = estimate_from_value(&parse(&plain), cfg())
        .unwrap()
        .input_tokens;
    let schema_tokens = estimate_from_value(&parse(&with_schema), cfg())
        .unwrap()
        .input_tokens;
    assert!(
        schema_tokens > plain_tokens,
        "schema {schema_tokens} should exceed plain {plain_tokens}"
    );
}

#[test]
fn non_object_body_is_fail_open() {
    let body = Bytes::from("[]");
    let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(estimate_from_value(&value, cfg()).is_none());
}

#[test]
fn margin_reduces_window() {
    let config = cfg();
    assert_eq!(config.apply_margin(131_072), 124_518);
    assert_eq!(config.apply_margin(0), 0);
}
