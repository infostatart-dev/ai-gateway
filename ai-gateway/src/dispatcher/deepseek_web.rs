use std::sync::Arc;

use bytes::Bytes;
use deepseek_web::{ExecuteRequest, Executor, TurnHook};
use http::{HeaderMap, StatusCode};
use http_body_util::BodyExt;
use serde_json::Value;
use tracing::Instrument;

use crate::{
    app_state::AppState,
    config::{credentials::ProviderCredentialId, deepseek_web as deepseek_cfg},
    dispatcher::service::{
        Dispatcher,
        outcome::{DispatchOutcome, outcome_from_bytes},
    },
    error::{api::ApiError, internal::InternalError},
    router::budget_aware::DeepSeekWebTrace,
    types::{provider::InferenceProvider, request::Request},
};

impl Dispatcher {
    #[tracing::instrument(name = "deepseek_web_execute", skip(self, req))]
    pub(super) async fn dispatch_deepseek_web(
        &self,
        req: Request,
        request_headers: HeaderMap,
        credential_id: Option<&ProviderCredentialId>,
    ) -> Result<DispatchOutcome, ApiError> {
        let session_path = resolve_session_path(
            &self.app_state.config().credentials,
            credential_id,
            deepseek_cfg::DEFAULT_CREDENTIAL_ID,
        )
        .ok_or_else(|| ApiError::Internal(InternalError::ProviderNotFound))?;
        let user_token = deepseek_cfg::load_session_token(&session_path)
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
        let stream = openai_body
            .get("stream")
            .and_then(Value::as_bool)
            .unwrap_or(false);

        let target_url = url::Url::parse(deepseek_web::COMPLETION_URL)
            .map_err(|_| InternalError::Internal)?;

        let turn_hook = turn_pacing_hook(
            self.app_state.clone(),
            self.provider.clone(),
            credential_id.cloned(),
        );

        let result = match Executor::default()
            .execute(ExecuteRequest {
                user_token,
                body: openai_body,
                stream,
                turn_hook: Some(turn_hook),
            })
            .instrument(tracing::info_span!("deepseek_web_executor"))
            .await
        {
            Ok(result) => result,
            Err(e) => {
                tracing::warn!(error = %e, "deepseek-web executor failed");
                let (status, body) = deepseek_error_body(e)?;
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

        tracing::info!(
            deepseek_web_turns = result.stats.turns,
            deepseek_web_upload_parts = result.stats.upload_parts,
            deepseek_web_pow_cache_hits = result.stats.pow_cache_hits,
            "deepseek-web executor stats"
        );

        let status = StatusCode::from_u16(result.status)
            .unwrap_or(StatusCode::BAD_GATEWAY);
        let mut headers = HeaderMap::new();
        if stream {
            headers.insert(
                http::header::CONTENT_TYPE,
                http::HeaderValue::from_static("text/event-stream"),
            );
        } else {
            headers.insert(
                http::header::CONTENT_TYPE,
                http::HeaderValue::from_static("application/json"),
            );
        }
        let response_body = Bytes::from(result.body);
        let mut outcome = outcome_from_bytes(
            status,
            headers,
            &response_body,
            target_url,
            req_body_bytes,
            request_headers,
        )
        .map_err(ApiError::Internal)?;
        outcome.response.extensions_mut().insert(DeepSeekWebTrace {
            turns: result.stats.turns,
            upload_parts: result.stats.upload_parts,
            pow_cache_hits: result.stats.pow_cache_hits,
        });
        Ok(outcome)
    }
}

fn turn_pacing_hook(
    app_state: AppState,
    provider: InferenceProvider,
    credential_id: Option<ProviderCredentialId>,
) -> TurnHook {
    Arc::new(move || {
        let app_state = app_state.clone();
        let provider = provider.clone();
        let credential_id = credential_id.clone();
        Box::pin(async move {
            crate::router::pacing::acquire_upstream_pacing(
                &app_state,
                &provider,
                credential_id.as_ref(),
                0,
            )
            .await
            .map_err(|e| deepseek_web::Error::Other(e.to_string()))?;
            Ok(())
        })
    })
}

fn resolve_session_path(
    registry: &crate::config::credentials::CredentialRegistry,
    credential_id: Option<&ProviderCredentialId>,
    default_id: &str,
) -> Option<std::path::PathBuf> {
    if let Some(id) = credential_id
        && let Some(cred) = registry.get(id)
        && let Some(path) = cred.key.as_secret()
    {
        return Some(std::path::PathBuf::from(path.expose()));
    }
    deepseek_cfg::session_path_for_credential(default_id)
}

fn deepseek_error_body(
    error: deepseek_web::Error,
) -> Result<(StatusCode, Bytes), InternalError> {
    let (status, message, error_type, code) = match error {
        deepseek_web::Error::SessionAuth(message) => (
            StatusCode::UNAUTHORIZED,
            message,
            "authentication_error",
            Some("invalid_session"),
        ),
        deepseek_web::Error::EmptyResponse => (
            StatusCode::BAD_GATEWAY,
            "empty response from DeepSeek".into(),
            "server_error",
            None,
        ),
        deepseek_web::Error::Other(message) => {
            (StatusCode::BAD_GATEWAY, message, "server_error", None)
        }
        deepseek_web::Error::Upstream { status, message } => (
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
    use deepseek_web::Error;

    use super::deepseek_error_body;

    #[test]
    fn session_auth_maps_to_401() {
        let (status, body) =
            deepseek_error_body(Error::SessionAuth("expired".into())).unwrap();
        assert_eq!(status, http::StatusCode::UNAUTHORIZED);
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["error"]["code"], "invalid_session");
    }

    #[test]
    fn upstream_error_preserves_status() {
        let (status, _) = deepseek_error_body(Error::Upstream {
            status: 429,
            message: "rate limited".into(),
        })
        .unwrap();
        assert_eq!(status, http::StatusCode::TOO_MANY_REQUESTS);
    }
}
