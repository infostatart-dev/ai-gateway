use super::{
    merge::merge_prompt_with_request, variables::process_prompt_variables,
    version::get_prompt_version,
};
use crate::{
    app_state::AppState,
    error::{
        api::ApiError, internal::InternalError,
        invalid_req::InvalidRequestError, prompts::PromptError,
    },
    store::minio::MinioClient,
    types::{
        extensions::{AuthContext, PromptContext},
        request::Request,
    },
};
use http_body_util::BodyExt;

pub async fn build_prompt_request(
    app_state: AppState,
    req: Request,
) -> Result<Request, ApiError> {
    let (parts, body) = req.into_parts();
    let body_bytes = body
        .collect()
        .await
        .map_err(InternalError::CollectBodyError)?
        .to_bytes();
    let request_json: serde_json::Value = serde_json::from_slice(&body_bytes)
        .map_err(|e| {
        ApiError::InvalidRequest(InvalidRequestError::InvalidRequestBody(e))
    })?;

    if request_json.pointer("/prompt_id").is_none() {
        return Ok(Request::from_parts(
            parts,
            axum_core::body::Body::from(body_bytes),
        ));
    }
    let mut prompt_ctx: PromptContext =
        serde_json::from_value(request_json.clone())
            .map_err(InvalidRequestError::from)?;
    let auth_ctx = parts
        .extensions
        .get::<AuthContext>()
        .cloned()
        .ok_or(InternalError::ExtensionNotFound("AuthContext"))?;

    let version_id = if let Some(ref vid) = prompt_ctx.prompt_version_id {
        vid.clone()
    } else {
        let v_resp = get_prompt_version(
            &app_state,
            &prompt_ctx.prompt_id,
            &auth_ctx,
        )
        .await?
        .data()
        .map_err(|e| {
            tracing::error!(error = %e, "failed to get production version");
            ApiError::Internal(InternalError::PromptError(
                PromptError::UnexpectedResponse(e),
            ))
        })?;
        prompt_ctx.prompt_version_id = Some(v_resp.id.clone());
        v_resp.id
    };

    let s3 = if app_state.config().deployment_target.is_cloud() {
        MinioClient::cloud(&app_state.0.minio)
    } else {
        MinioClient::sidecar(&app_state.0.jawn_http_client)
    };
    let p_body = s3
        .pull_prompt_body(
            &app_state,
            &auth_ctx,
            &prompt_ctx.prompt_id,
            &version_id,
        )
        .await
        .map_err(|e| ApiError::Internal(InternalError::PromptError(e)))?;
    let merged = merge_prompt_with_request(p_body, &request_json)?;
    let processed = process_prompt_variables(merged, &prompt_ctx)?;
    let mut parts = parts;
    parts.extensions.insert(prompt_ctx);
    Ok(Request::from_parts(
        parts,
        axum_core::body::Body::from(
            serde_json::to_vec(&processed)
                .map_err(|_| InternalError::Internal)?,
        ),
    ))
}
