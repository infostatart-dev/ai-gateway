use super::{Dispatcher, retry::dispatch_stream_with_retry};
use crate::{
    dispatcher::{client::ProviderClient, extensions::ExtensionsCopier},
    error::{api::ApiError, internal::InternalError},
    types::{body::Body, request::Request},
};
use http::{HeaderName, HeaderValue};
use http_body_util::BodyExt;
use std::str::FromStr;
use tracing::{Instrument, info_span};
use uuid::Uuid;

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
        ) = Self::extract_request_context(&mut req)?;
        let auth_ctx = req_ctx.auth_context.as_ref();
        let target_provider = &self.provider;

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
            &req_ctx,
            target_provider,
            extracted_path_and_query.as_str(),
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
        if let Some(ref api_endpoint) = api_endpoint {
            let endpoint_metrics = self
                .app_state
                .0
                .endpoint_metrics
                .health_metrics(api_endpoint.clone())?;
            endpoint_metrics.incr_req_count();
        }

        let (mut client_response, response_body_for_logger, tfft_rx) =
            if mapper_ctx.is_stream {
                dispatch_stream_with_retry(
                    &self.app_state,
                    request_builder,
                    req_body_bytes.clone(),
                    api_endpoint.clone(),
                    metrics_for_stream,
                    &req_ctx,
                    request_kind,
                )
                .await?
            } else {
                self.dispatch_sync_with_retry(
                    request_builder,
                    req_body_bytes.clone(),
                    &req_ctx,
                    request_kind,
                )
                .instrument(info_span!("dispatch_sync"))
                .await?
            };

        tracing::info!(method = %method, target_url = %target_url, is_stream = %mapper_ctx.is_stream, response_status = %client_response.status(), "proxied request");
        let helicone_request_id = Uuid::new_v4();
        let provider_request_id = {
            let headers = client_response.headers_mut();
            headers.insert(
                "helicone-id",
                HeaderValue::from_str(&helicone_request_id.to_string())
                    .expect("valid uuid"),
            );
            headers.remove(http::header::CONTENT_LENGTH);
            headers.remove("x-request-id")
        };

        let extensions_copier = ExtensionsCopier::builder()
            .inference_provider(inference_provider)
            .router_id(router_id.clone())
            .auth_context(auth_ctx.cloned())
            .provider_request_id(provider_request_id)
            .mapper_ctx(mapper_ctx.clone())
            .build();
        extensions_copier.copy_extensions(client_response.extensions_mut());
        client_response.extensions_mut().insert(mapper_ctx.clone());
        if let Some(api_endpoint) = api_endpoint.clone() {
            client_response.extensions_mut().insert(api_endpoint);
        }
        client_response
            .extensions_mut()
            .insert(extracted_path_and_query);

        let response_status = client_response.status();
        let response_headers = client_response.headers();
        self.handle_error_and_rate_limiting(
            response_status,
            response_headers,
            api_endpoint.clone(),
            mapper_ctx.model.clone(),
        )
        .await?;

        self.handle_logging(
            &req_ctx,
            start_time,
            start_instant,
            target_url,
            headers,
            req_body_bytes,
            &client_response,
            response_body_for_logger,
            tfft_rx,
            &mapper_ctx,
            router_id,
            helicone_request_id,
            prompt_ctx,
        );

        Ok(client_response)
    }
}
