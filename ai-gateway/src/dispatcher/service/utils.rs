use std::sync::Arc;

use chrono::{DateTime, Utc};
use http::{HeaderMap, StatusCode};
use tokio::time::Instant;

use super::Dispatcher;
use crate::{
    endpoints::ApiEndpoint,
    error::{api::ApiError, internal::InternalError},
    types::{
        extensions::{
            MapperContext, PromptContext, RequestContext, RequestKind,
            RouterRuntimeLabels,
        },
        model_id::ModelId,
        provider::InferenceProvider,
        rate_limit::RateLimitEvent,
        request::Request,
        router::RouterId,
    },
};

impl Dispatcher {
    #[allow(clippy::type_complexity, clippy::too_many_arguments)]
    pub fn extract_request_context(
        req: &mut Request,
    ) -> Result<
        (
            MapperContext,
            Arc<RequestContext>,
            Option<ApiEndpoint>,
            http::uri::PathAndQuery,
            InferenceProvider,
            Option<RouterId>,
            Instant,
            DateTime<Utc>,
            RequestKind,
            Option<PromptContext>,
            Option<RouterRuntimeLabels>,
        ),
        ApiError,
    > {
        let router_runtime_labels =
            req.extensions().get::<RouterRuntimeLabels>().cloned();
        let mapper_ctx = req
            .extensions_mut()
            .remove::<MapperContext>()
            .ok_or(InternalError::ExtensionNotFound("MapperContext"))?;
        let req_ctx = req
            .extensions_mut()
            .remove::<Arc<RequestContext>>()
            .ok_or(InternalError::ExtensionNotFound("RequestContext"))?;
        let api_endpoint = req.extensions().get::<ApiEndpoint>().cloned();
        let extracted_path_and_query = req
            .extensions_mut()
            .remove::<http::uri::PathAndQuery>()
            .ok_or(ApiError::Internal(InternalError::ExtensionNotFound(
                "PathAndQuery",
            )))?;
        let inference_provider = req
            .extensions()
            .get::<InferenceProvider>()
            .cloned()
            .ok_or(InternalError::ExtensionNotFound("InferenceProvider"))?;
        let router_id = req.extensions().get::<RouterId>().cloned();
        let start_instant = req
            .extensions()
            .get::<Instant>()
            .copied()
            .unwrap_or_else(Instant::now);
        let start_time = req
            .extensions()
            .get::<DateTime<Utc>>()
            .copied()
            .unwrap_or_else(Utc::now);
        let request_kind = req
            .extensions()
            .get::<RequestKind>()
            .copied()
            .ok_or(InternalError::ExtensionNotFound("RequestKind"))?;
        let prompt_ctx = req.extensions_mut().remove::<PromptContext>();
        Ok((
            mapper_ctx,
            req_ctx,
            api_endpoint,
            extracted_path_and_query,
            inference_provider,
            router_id,
            start_instant,
            start_time,
            request_kind,
            prompt_ctx,
            router_runtime_labels,
        ))
    }

    pub async fn handle_error_and_rate_limiting(
        &self,
        status: StatusCode,
        headers: &HeaderMap,
        api_endpoint: Option<ApiEndpoint>,
        model_id: Option<ModelId>,
    ) -> Result<(), ApiError> {
        if status.is_server_error() {
            if let Some(ep) = api_endpoint {
                self.app_state
                    .0
                    .endpoint_metrics
                    .health_metrics(ep)?
                    .incr_remote_internal_error_count();
            }
        } else if status == StatusCode::TOO_MANY_REQUESTS
            && let Some(ref ep) = api_endpoint
        {
            let retry_after = extract_retry_after(headers);
            if let Some(tx) = &self.rate_limit_tx {
                let mut event = RateLimitEvent::new(ep.clone(), retry_after);
                if let Some(m) = model_id {
                    event = event.with_model_id(m);
                }
                let _ = tx.send(event).await;
            }
        }
        Ok(())
    }

    pub fn build_target_url(
        &self,
        req_ctx: &RequestContext,
        target_provider: &InferenceProvider,
        path: &str,
    ) -> Result<url::Url, ApiError> {
        let config = self.app_state.config();
        let base_url = req_ctx
            .router_config
            .as_ref()
            .and_then(|c| c.providers.as_ref())
            .and_then(|p| p.get(target_provider))
            .map(|p| &p.base_url)
            .or_else(|| {
                config.providers.get(target_provider).map(|p| &p.base_url)
            })
            .ok_or_else(|| {
                InternalError::ProviderNotConfigured(target_provider.clone())
            })?;
        Ok(crate::dispatcher::cloudflare_url::join_provider_path(
            target_provider,
            base_url,
            path,
            cloudflare_account_id(config, None).as_deref(),
        )?)
    }
}

fn cloudflare_account_id(
    config: &crate::config::Config,
    credential_id: Option<&crate::config::credentials::ProviderCredentialId>,
) -> Option<String> {
    if let Some(id) = credential_id {
        if let Some(from_secrets) =
            crate::config::secrets_file::SecretsFile::cloudflare_account_id(
                id.as_str(),
            )
        {
            return Some(from_secrets);
        }
        if let Some(cred) = config.credentials.get(id)
            && let Some(secret) = cred.key.as_secret()
            && let Some((account_id, _)) =
                crate::config::cloudflare::parse_combined(secret.expose())
        {
            return Some(account_id);
        }
    }
    crate::config::secrets_file::SecretsFile::cloudflare_account_id(
        "cloudflare-default",
    )
}

pub fn extract_retry_after(headers: &HeaderMap) -> Option<u64> {
    crate::router::retry_after::extract_retry_after_from_headers(headers)
}
