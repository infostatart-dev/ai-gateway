use super::snapshot::KeyInfoSnapshot;

#[must_use]
pub fn parse_openrouter_key_info(body: &[u8]) -> Option<KeyInfoSnapshot> {
    let value: serde_json::Value = serde_json::from_slice(body).ok()?;
    let data = value.get("data")?;
    let limit_remaining = data.get("limit_remaining").and_then(json_f64);
    let is_free_tier = data
        .get("is_free_tier")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    Some(KeyInfoSnapshot {
        limit_remaining,
        is_free_tier,
        probed_at: std::time::Instant::now(),
    })
}

#[allow(clippy::cast_precision_loss)]
fn json_f64(value: &serde_json::Value) -> Option<f64> {
    if let Some(v) = value.as_f64() {
        return Some(v);
    }
    value.as_u64().map(|v| v as f64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_zero_remaining_free_tier() {
        let body = br#"{"data":{"limit_remaining":0,"is_free_tier":true}}"#;
        let snap = parse_openrouter_key_info(body).unwrap();
        assert_eq!(snap.limit_remaining, Some(0.0));
        assert!(snap.is_free_tier);
        assert!(snap.blocks_paid_route("openai/gpt-4o"));
        assert!(!snap.blocks_paid_route("qwen/qwen3:free"));
    }

    #[test]
    fn zero_remaining_blocks_paid_route() {
        let body = br#"{"data":{"limit_remaining":0,"is_free_tier":false}}"#;
        let snap = parse_openrouter_key_info(body).unwrap();
        assert!(snap.blocks_paid_route("openai/gpt-4o"));
        assert!(!snap.blocks_paid_route("qwen/qwen3:free"));
    }
}
