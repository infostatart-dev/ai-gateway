use http_body_util::BodyExt;

use super::token_policy;
use crate::{
    endpoints::{ApiEndpoint, EndpointType},
    error::{
        api::ApiError, internal::InternalError,
        invalid_req::InvalidRequestError,
    },
    middleware::decision::policy::KeyPolicy,
    types::request::Request,
};

pub(super) struct PreparedDecisionRequest {
    pub request: Request,
    pub reserved_output_tokens: i64,
}

pub(super) async fn prepare_request(
    req: Request,
    policy: &KeyPolicy,
) -> Result<PreparedDecisionRequest, ApiError> {
    let applies_token_policy = req
        .extensions()
        .get::<ApiEndpoint>()
        .is_none_or(|endpoint| endpoint.endpoint_type() == EndpointType::Chat);
    let (mut parts, body) = req.into_parts();
    let body_bytes = body
        .collect()
        .await
        .map_err(|error| {
            ApiError::Internal(InternalError::CollectBodyError(error))
        })?
        .to_bytes();
    let mut modified_body = body_bytes.clone();
    let mut reserved_output_tokens = u64::from(policy.max_output_tokens);

    if applies_token_policy {
        let policy_result =
            token_policy::apply_token_policy(&body_bytes, policy)?;
        reserved_output_tokens = policy_result.reserved_output_tokens;
        if let Some(body) = policy_result.modified_body {
            parts.headers.remove(http::header::CONTENT_LENGTH);
            modified_body = body;
        }
    }

    Ok(PreparedDecisionRequest {
        request: Request::from_parts(
            parts,
            axum_core::body::Body::from(modified_body),
        ),
        reserved_output_tokens: i64::try_from(reserved_output_tokens).map_err(
            |_| {
                ApiError::InvalidRequest(InvalidRequestError::BudgetExceeded(
                    "max_tokens exceeds supported budget range".to_string(),
                ))
            },
        )?,
    })
}
