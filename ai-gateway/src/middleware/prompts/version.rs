use super::types::Prompt2025Version;
use crate::{
    app_state::AppState,
    error::{api::ApiError, internal::InternalError, prompts::PromptError},
    types::{extensions::AuthContext, response::JawnResponse},
};

pub async fn get_prompt_version(
    app_state: &AppState,
    prompt_id: &str,
    auth_ctx: &AuthContext,
) -> Result<JawnResponse<Prompt2025Version>, ApiError> {
    let url = app_state
        .config()
        .helicone
        .base_url
        .join("/v1/prompt-2025/query/production-version")
        .map_err(|_| InternalError::Internal)?;
    let resp = app_state
        .0
        .jawn_http_client
        .request_client
        .post(url)
        .json(&serde_json::json!({ "promptId": prompt_id }))
        .header(
            "authorization",
            format!("Bearer {}", auth_ctx.api_key.expose()),
        )
        .send()
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to get prompt version");
            ApiError::Internal(InternalError::PromptError(
                PromptError::FailedToGetProductionVersion(e),
            ))
        })?
        .error_for_status()
        .map_err(|e| {
            ApiError::Internal(InternalError::PromptError(
                PromptError::FailedToGetProductionVersion(e),
            ))
        })?;
    resp.json::<JawnResponse<Prompt2025Version>>()
        .await
        .map_err(|e| {
            ApiError::Internal(InternalError::PromptError(
                PromptError::FailedToGetProductionVersion(e),
            ))
        })
}
