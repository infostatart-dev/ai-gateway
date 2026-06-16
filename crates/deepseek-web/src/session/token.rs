/// DeepSeek stores `userToken` as a raw string or JSON `{"value":"..."}`.
#[must_use]
pub fn normalize_user_token(raw: &str) -> String {
    let trimmed = raw.trim();
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(trimmed)
        && let Some(inner) =
            value.get("value").and_then(serde_json::Value::as_str)
    {
        return inner.to_string();
    }
    trimmed.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unwraps_json_value_wrapper() {
        let raw = r#"{"value":"tok_abc"}"#;
        assert_eq!(normalize_user_token(raw), "tok_abc");
    }

    #[test]
    fn passes_through_plain_token() {
        assert_eq!(normalize_user_token("plain-token"), "plain-token");
    }
}
