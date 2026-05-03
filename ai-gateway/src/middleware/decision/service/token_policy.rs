use bytes::Bytes;
use serde_json::Value;

use crate::{
    error::{api::ApiError, invalid_req::InvalidRequestError},
    middleware::decision::policy::KeyPolicy,
};

pub(super) struct TokenPolicyResult {
    pub reserved_output_tokens: u64,
    pub modified_body: Option<Bytes>,
}

pub(super) fn apply_token_policy(
    body: &Bytes,
    policy: &KeyPolicy,
) -> Result<TokenPolicyResult, ApiError> {
    let output_cap = u64::from(policy.max_output_tokens);
    let Ok(mut json) = serde_json::from_slice::<Value>(body) else {
        return Ok(TokenPolicyResult {
            reserved_output_tokens: output_cap,
            modified_body: None,
        });
    };
    let Some(obj) = json.as_object_mut() else {
        return Ok(TokenPolicyResult {
            reserved_output_tokens: output_cap,
            modified_body: None,
        });
    };

    if let Some(requested_output_tokens) = requested_output_tokens(obj) {
        if requested_output_tokens > output_cap {
            return Err(ApiError::InvalidRequest(
                InvalidRequestError::BudgetExceeded(format!(
                    "max_tokens exceeds budget cap of {output_cap}"
                )),
            ));
        }
        return Ok(TokenPolicyResult {
            reserved_output_tokens: requested_output_tokens,
            modified_body: None,
        });
    }

    obj.insert("max_tokens".to_string(), serde_json::json!(output_cap));
    let modified_body = serde_json::to_vec(&json)
        .map(Bytes::from)
        .map_err(InvalidRequestError::InvalidRequestBody)?;
    Ok(TokenPolicyResult {
        reserved_output_tokens: output_cap,
        modified_body: Some(modified_body),
    })
}

fn requested_output_tokens(
    obj: &serde_json::Map<String, Value>,
) -> Option<u64> {
    obj.get("max_tokens")
        .or_else(|| obj.get("max_completion_tokens"))
        .and_then(Value::as_u64)
}
