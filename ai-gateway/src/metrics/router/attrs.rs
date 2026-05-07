use http::StatusCode;
use opentelemetry::KeyValue;

use crate::types::extensions::RouterRuntimeLabels;

#[must_use]
pub fn base_router_kv(rtl: &RouterRuntimeLabels) -> Vec<KeyValue> {
    vec![
        KeyValue::new("router_id", rtl.router_id.to_string()),
        KeyValue::new("endpoint_type", rtl.endpoint_type.clone()),
        KeyValue::new("strategy", rtl.strategy),
    ]
}

#[must_use]
pub fn status_class(status: StatusCode) -> &'static str {
    if status.is_informational() {
        "1xx"
    } else if status.is_success() {
        "2xx"
    } else if status.is_redirection() {
        "3xx"
    } else if status.is_client_error() {
        "4xx"
    } else if status.is_server_error() {
        "5xx"
    } else {
        "unknown"
    }
}
