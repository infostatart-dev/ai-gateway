use serde_json::Value;

#[must_use]
pub fn looks_like_unsupported_model(body: Option<&[u8]>) -> bool {
    let text = body_to_lower_text(body);
    text.contains("unsupported model")
}

#[must_use]
pub fn looks_like_unpaid_route(body: Option<&[u8]>) -> bool {
    let text = body_to_lower_text(body);
    text.contains("never purchased")
        || text.contains("purchase credits")
        || text.contains("haven't purchased")
        || text.contains("have not purchased")
        || text.contains("not purchased credits")
}

#[must_use]
pub fn looks_like_high_demand(body: Option<&[u8]>) -> bool {
    let text = body_to_lower_text(body);
    text.contains("high demand")
        || (text.contains("try again later") && text.contains("model"))
}

#[must_use]
pub fn looks_like_abuse_block(body: Option<&[u8]>) -> bool {
    let text = body_to_lower_text(body);
    if text.is_empty() {
        return false;
    }

    if text.contains("unusual activity") || text.contains("detected unusual") {
        return true;
    }

    if text.contains("sentinel") && text.contains("blocked") {
        return true;
    }

    if text.contains("try again later")
        && (text.contains("unusual") || text.contains("detected"))
    {
        return true;
    }

    false
}

fn body_to_lower_text(body: Option<&[u8]>) -> String {
    let Some(bytes) = body else {
        return String::new();
    };
    if let Ok(value) = serde_json::from_slice::<Value>(bytes) {
        return value.to_string().to_ascii_lowercase();
    }
    String::from_utf8_lossy(bytes).to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unsupported_model_body_triggers_auth_cooldown() {
        let body = br#"{"error":{"message":"Unsupported model (model=LongCat-Flash-Lite)"}}"#;
        assert!(looks_like_unsupported_model(Some(body)));
    }

    #[test]
    fn unusual_activity_body_is_abuse() {
        let body = b"Our systems have detected unusual activity from your \
                    network.";
        assert!(looks_like_abuse_block(Some(body)));
    }

    #[test]
    fn sentinel_blocked_message_is_abuse() {
        let body = b"Sentinel /prepare blocked (HTTP 403)";
        assert!(looks_like_abuse_block(Some(body)));
    }

    #[test]
    fn generic_502_is_not_abuse() {
        assert!(!looks_like_abuse_block(Some(b"upstream connection reset")));
    }

    #[test]
    fn plain_try_again_later_is_not_abuse() {
        assert!(!looks_like_abuse_block(Some(
            b"Service unavailable. Please try again later."
        )));
    }

    #[test]
    fn high_demand_body_is_detected() {
        let body = b"This model is currently experiencing high demand.";
        assert!(looks_like_high_demand(Some(body)));
    }

    #[test]
    fn never_purchased_credits_is_unpaid_route() {
        let body = br#"{"error":{"message":"You have never purchased credits. Only free models are available."}}"#;
        assert!(looks_like_unpaid_route(Some(body)));
    }

    #[test]
    fn billing_cap_is_not_unpaid_route() {
        let body = br#"{"error":{"message":"Set up billing to continue."}}"#;
        assert!(!looks_like_unpaid_route(Some(body)));
    }
}
