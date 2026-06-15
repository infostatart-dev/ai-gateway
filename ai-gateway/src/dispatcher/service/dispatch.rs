use std::str::FromStr;

use http::{HeaderName, HeaderValue};
use http_body_util::BodyExt;
use tracing::{Instrument, info_span};

use super::{
    Dispatcher,
    outcome::{DispatchOutcome, FinalizeDispatchContext},
    retry::dispatch_stream_with_retry,
};
use crate::{
    config::credentials::ProviderCredentialId,
    dispatcher::client::ProviderClient,
    error::{api::ApiError, internal::InternalError},
    types::{body::Body, request::Request},
};

struct UpstreamProxyDispatch<'a> {
    req: Request,
    auth_ctx: Option<&'a crate::types::extensions::AuthContext>,
    target_provider: &'a crate::types::provider::InferenceProvider,
    extracted_path_and_query: &'a str,
    is_stream: bool,
    api_endpoint: Option<crate::endpoints::ApiEndpoint>,
    router_runtime_labels:
        Option<crate::types::extensions::RouterRuntimeLabels>,
    req_ctx: &'a crate::types::extensions::RequestContext,
    request_kind: crate::types::extensions::RequestKind,
}

impl Dispatcher {
    #[allow(clippy::too_many_lines)]
    pub async fn dispatch(
        &self,
        mut req: Request,
    ) -> Result<http::Response<Body>, ApiError> {
        let (
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
        ) = Self::extract_request_context(&mut req)?;
        let auth_ctx = req_ctx.auth_context.as_ref();
        let target_provider = &self.provider;

        let finalize_ctx = FinalizeDispatchContext {
            mapper_ctx: mapper_ctx.clone(),
            req_ctx: &req_ctx,
            api_endpoint: api_endpoint.clone(),
            inference_provider: inference_provider.clone(),
            router_id: router_id.clone(),
            start_instant,
            start_time,
            request_kind,
            prompt_ctx,
            router_runtime_labels: router_runtime_labels.clone(),
            extracted_path_and_query: extracted_path_and_query.clone(),
        };

        if let Some(ref api_endpoint) = api_endpoint {
            let endpoint_metrics = self
                .app_state
                .0
                .endpoint_metrics
                .health_metrics(api_endpoint.clone())?;
            endpoint_metrics.incr_req_count();
        }

        let credential_id = req.extensions().get::<ProviderCredentialId>().cloned();
        let _pacing_permit = crate::router::pacing::acquire_upstream_pacing(
            &self.app_state,
            target_provider,
            credential_id.as_ref(),
        )
        .await?;

        let outcome =
            if crate::config::chatgpt_web::is_chatgpt_web(target_provider) {
                let headers = req.headers().clone();
                self.dispatch_chatgpt_web(req, headers).await?
            } else {
                self.dispatch_via_upstream_proxy(UpstreamProxyDispatch {
                    req,
                    auth_ctx,
                    target_provider,
                    extracted_path_and_query: extracted_path_and_query.as_str(),
                    is_stream: mapper_ctx.is_stream,
                    api_endpoint: api_endpoint.clone(),
                    router_runtime_labels: router_runtime_labels.clone(),
                    req_ctx: &req_ctx,
                    request_kind,
                })
                .await?
            };

        self.finalize_dispatch(outcome, finalize_ctx).await
    }

    async fn dispatch_via_upstream_proxy(
        &self,
        dispatch: UpstreamProxyDispatch<'_>,
    ) -> Result<DispatchOutcome, ApiError> {
        let UpstreamProxyDispatch {
            mut req,
            auth_ctx,
            target_provider,
            extracted_path_and_query,
            is_stream,
            api_endpoint,
            router_runtime_labels,
            req_ctx,
            request_kind,
        } = dispatch;
        {
            let h = req.headers_mut();
            h.remove(http::header::HOST);
            h.remove(http::header::AUTHORIZATION);
            h.remove(http::header::CONTENT_LENGTH);
            h.remove(HeaderName::from_str("helicone-api-key").unwrap());
            h.remove(http::header::ACCEPT_ENCODING);
            h.insert(
                http::header::ACCEPT_ENCODING,
                HeaderValue::from_static("identity"),
            );
        }

        let method = req.method().clone();
        let headers = req.headers().clone();
        let target_url = self.build_target_url(
            req_ctx,
            target_provider,
            extracted_path_and_query,
        )?;
        let req_body_bytes = req
            .into_body()
            .collect()
            .await
            .map_err(|e| InternalError::RequestBodyError(Box::new(e)))?
            .to_bytes();
        let request_builder = self
            .client
            .as_ref()
            .request(method.clone(), target_url.clone())
            .headers(headers.clone());
        let request_builder = self
            .client
            .authenticate(
                &self.app_state,
                request_builder,
                &req_body_bytes,
                auth_ctx,
                self.provider.clone(),
            )
            .await?;

        let metrics_for_stream = self.app_state.0.endpoint_metrics.clone();
        let (client_response, response_body_for_logger, tfft_rx) = if is_stream
        {
            dispatch_stream_with_retry(
                &self.app_state,
                self.provider.clone(),
                router_runtime_labels,
                request_builder,
                req_body_bytes.clone(),
                api_endpoint,
                metrics_for_stream,
                req_ctx,
                request_kind,
            )
            .await?
        } else {
            self.dispatch_sync_with_retry(
                request_builder,
                req_body_bytes.clone(),
                req_ctx,
                request_kind,
                router_runtime_labels,
            )
            .instrument(info_span!("dispatch_sync"))
            .await?
        };

        Ok(DispatchOutcome {
            response: client_response,
            body_reader: response_body_for_logger,
            tfft_rx,
            target_url,
            req_body_bytes,
            request_headers: headers,
        })
    }
}
