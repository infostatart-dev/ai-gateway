use serde_json::Value;

use crate::app_state::AppState;

#[must_use]
pub fn credential_attempts(app_state: &AppState, credential: &str) -> u64 {
    app_state
        .provider_stats_snapshot(None, Some(credential))
        .providers
        .into_iter()
        .find(|row| row.credential == credential)
        .map_or(0, |row| row.calls.attempts)
}

pub fn attempts_for_credential(snapshot: &Value, credential: &str) -> u64 {
    snapshot
        .get("providers")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .find(|row| {
            row.get("credential").and_then(Value::as_str) == Some(credential)
        })
        .and_then(|row| row.get("calls"))
        .and_then(|calls| calls.get("attempts"))
        .and_then(Value::as_u64)
        .unwrap_or(0)
}

pub fn assert_zero_attempts(snapshot: &Value, credential: &str) {
    assert_eq!(
        attempts_for_credential(snapshot, credential),
        0,
        "expected zero attempts for {credential}"
    );
}

pub fn failover_rate(snapshot: &Value) -> f64 {
    snapshot
        .get("routing")
        .and_then(|r| r.get("failover_rate"))
        .and_then(Value::as_f64)
        .unwrap_or(0.0)
}

pub fn total_client_requests(snapshot: &Value) -> u64 {
    snapshot
        .get("routing")
        .and_then(|r| r.get("client_requests"))
        .and_then(Value::as_u64)
        .unwrap_or(0)
}
