use std::path::PathBuf;

use bytes::Bytes;
use chatgpt_web::{CONV_URL, ExecuteRequest, Executor};
use http::{HeaderMap, StatusCode};
use http_body_util::BodyExt;
use serde_json::Value;
use tracing::Instrument;
use url::Url;

use crate::{
    config::{chatgpt_web as chatgpt_cfg, credentials::ProviderCredentialId},
    dispatcher::service::{
        Dispatcher,
        outcome::{DispatchOutcome, outcome_from_bytes},
    },
    error::{api::ApiError, internal::InternalError},
    types::request::Request,
};

impl Dispatcher {
    #[tracing::instrument(name = "chatgpt_web_execute", skip(self, req))]
    pub(super) async fn dispatch_chatgpt_web(
        &self,
        req: Request,
        request_headers: HeaderMap,
        credential_id: Option<&ProviderCredentialId>,
    ) -> Result<DispatchOutcome, ApiError> {
        let session_path = resolve_session_path(
            &self.app_state.config().credentials,
            credential_id,
            chatgpt_cfg::DEFAULT_CREDENTIAL_ID,
        )
        .ok_or_else(|| ApiError::Internal(InternalError::ProviderNotFound))?;
        let cookie = chatgpt_cfg::load_session_cookie(&session_path)
            .ok_or_else(|| {
                ApiError::Internal(InternalError::ProviderNotFound)
            })?;
        let body_bytes = req
            .into_body()
            .collect()
            .await
            .map_err(|e| InternalError::RequestBodyError(Box::new(e)))?
            .to_bytes();
        let req_body_bytes = body_bytes.clone();
        let openai_body: Value =
            serde_json::from_slice(&body_bytes).map_err(|e| {
                ApiError::Internal(InternalError::Deserialize {
                    ty: "chat completion request",
                    error: e,
                })
            })?;
        let json_schema_required =
            chatgpt_cfg::request_requires_json_schema(&openai_body);

        let target_url =
            Url::parse(CONV_URL).map_err(|_| InternalError::Internal)?;

        let result = match Executor::default()
            .execute(ExecuteRequest {
                cookie,
                body: openai_body,
                json_schema_required,
                session_path: Some(session_path),
            })
            .instrument(tracing::info_span!("chatgpt_web_executor"))
            .await
        {
            Ok(result) => result,
            Err(e) => {
                tracing::warn!(error = %e, "chatgpt-web executor failed");
                let (status, body) = chatgpt_error_body(e)?;
                return outcome_from_bytes(
                    status,
                    HeaderMap::new(),
                    &body,
                    target_url,
                    req_body_bytes,
                    request_headers,
                )
                .map_err(ApiError::Internal);
            }
        };

        let status = StatusCode::from_u16(result.status)
            .unwrap_or(StatusCode::BAD_GATEWAY);
        let mut headers = HeaderMap::new();
        headers.insert(
            http::header::CONTENT_TYPE,
            http::HeaderValue::from_static("application/json"),
        );
        let response_body = Bytes::from(result.body);
        outcome_from_bytes(
            status,
            headers,
            &response_body,
            target_url,
            req_body_bytes,
            request_headers,
        )
        .map_err(ApiError::Internal)
    }
}

fn resolve_session_path(
    registry: &crate::config::credentials::CredentialRegistry,
    credential_id: Option<&ProviderCredentialId>,
    default_id: &str,
) -> Option<PathBuf> {
    if let Some(id) = credential_id
        && let Some(cred) = registry.get(id)
        && let Some(path) = cred.key.as_secret()
    {
        return Some(PathBuf::from(path.expose()));
    }
    chatgpt_cfg::session_path_for_credential(default_id)
}

fn chatgpt_error_body(
    error: chatgpt_web::Error,
) -> Result<(StatusCode, Bytes), InternalError> {
    let (status, message, error_type, code) = match error {
        chatgpt_web::Error::SessionAuth(message) => (
            StatusCode::UNAUTHORIZED,
            message,
            "authentication_error",
            Some("invalid_session"),
        ),
        chatgpt_web::Error::Other(message)
        | chatgpt_web::Error::SentinelBlocked(message) => {
            (StatusCode::BAD_GATEWAY, message, "server_error", None)
        }
        chatgpt_web::Error::Upstream { status, message } => (
            StatusCode::from_u16(status).unwrap_or(StatusCode::BAD_GATEWAY),
            message,
            "server_error",
            None,
        ),
        _ => return Err(InternalError::Internal),
    };
    let body = serde_json::json!({
        "error": {
            "message": message,
            "type": error_type,
            "param": null,
            "code": code,
        }
    });
    Ok((status, Bytes::from(body.to_string())))
}

#[cfg(test)]
mod tests {
    use chatgpt_web::Error;

    use super::chatgpt_error_body;

    #[test]
    fn session_auth_maps_to_401() {
        let (status, body) =
            chatgpt_error_body(Error::SessionAuth("expired".into())).unwrap();
        assert_eq!(status, http::StatusCode::UNAUTHORIZED);
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["error"]["code"], "invalid_session");
    }

    #[test]
    fn upstream_error_preserves_status() {
        let (status, _) = chatgpt_error_body(Error::Upstream {
            status: 429,
            message: "rate limited".into(),
        })
        .unwrap();
        assert_eq!(status, http::StatusCode::TOO_MANY_REQUESTS);
    }
}
