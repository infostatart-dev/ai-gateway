use bytes::Bytes;
use serde_json::json;

use super::token_policy::apply_token_policy;
use crate::{
    error::{api::ApiError, invalid_req::InvalidRequestError},
    middleware::decision::policy::{KeyPolicy, Tier},
};

fn policy(max_output_tokens: u32) -> KeyPolicy {
    KeyPolicy {
        tier: Tier::Free,
        budget_namespace: "test".to_string(),
        max_output_tokens,
        allow_hedging: false,
        allow_delay: false,
    }
}

#[test]
fn token_policy_injects_missing_max_tokens() {
    let body = Bytes::from(json!({ "model": "gpt-4o-mini" }).to_string());

    let result = apply_token_policy(&body, &policy(128)).unwrap();

    assert_eq!(result.reserved_output_tokens, 128);
    let modified = result.modified_body.unwrap();
    let value: serde_json::Value = serde_json::from_slice(&modified).unwrap();
    assert_eq!(value["max_tokens"], 128);
}

#[test]
fn token_policy_rejects_excessive_max_tokens() {
    let body = Bytes::from(
        json!({ "model": "gpt-4o-mini", "max_tokens": 129 }).to_string(),
    );

    let result = apply_token_policy(&body, &policy(128));

    assert!(matches!(
        result,
        Err(ApiError::InvalidRequest(
            InvalidRequestError::BudgetExceeded(_)
        ))
    ));
}
