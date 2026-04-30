use std::time::Duration;
use bytes::Bytes;
use http::{HeaderMap, HeaderValue};
use backon::{BackoffBuilder, ConstantBuilder, ExponentialBuilder, Retryable};
use reqwest::RequestBuilder;
use rust_decimal::prelude::ToPrimitive;
use crate::{
    app_state::AppState,
    config::retry::RetryConfig,
    discover::monitor::metrics::EndpointMetricsRegistry,
    endpoints::ApiEndpoint,
    error::{api::ApiError, internal::InternalError},
    types::{extensions::{RequestContext, RequestKind}, body::{Body, BodyReader}},
};
use super::Dispatcher;

impl Dispatcher {
    pub async fn dispatch_sync_with_retry(&self, request_builder: RequestBuilder, req_body_bytes: Bytes, req_ctx: &RequestContext, request_kind: RequestKind) -> Result<(http::Response<Body>, BodyReader, tokio::sync::oneshot::Receiver<()>), ApiError> {
        let retry_config = get_retry_config(&self.app_state, request_kind, req_ctx);
        if let Some(retry_config) = retry_config {
            match retry_config {
                RetryConfig::Exponential { min_delay, max_delay, max_retries, factor } => {
                    let retry_strategy = ExponentialBuilder::default().with_max_delay(*max_delay).with_min_delay(*min_delay).with_max_times(usize::from(*max_retries)).with_factor(factor.to_f32().unwrap_or(crate::config::retry::DEFAULT_RETRY_FACTOR)).with_jitter().build();
                    crate::utils::retry::RetryWithResult::new(|| async { Self::dispatch_sync(&request_builder, req_body_bytes.clone()).await }, retry_strategy)
                    .when(is_retryable_result).notify(notify_retry).await
                }
                RetryConfig::Constant { delay, max_retries } => {
                    let retry_strategy = ConstantBuilder::default().with_delay(*delay).with_max_times(usize::from(*max_retries)).with_jitter().build();
                    crate::utils::retry::RetryWithResult::new(|| async { Self::dispatch_sync(&request_builder, req_body_bytes.clone()).await }, retry_strategy)
                    .when(is_retryable_result).notify(notify_retry).await
                }
            }
        } else { Self::dispatch_sync(&request_builder, req_body_bytes).await }
    }
}

pub async fn dispatch_stream_with_retry(app_state: &AppState, request_builder: RequestBuilder, req_body_bytes: Bytes, api_endpoint: Option<ApiEndpoint>, metrics_registry: EndpointMetricsRegistry, request_ctx: &RequestContext, request_kind: RequestKind) -> Result<(http::Response<Body>, BodyReader, tokio::sync::oneshot::Receiver<()>), ApiError> {
    let retry_config = get_retry_config(app_state, request_kind, request_ctx);
    if let Some(retry_config) = retry_config {
        match retry_config {
            RetryConfig::Exponential { min_delay, max_delay, max_retries, factor } => {
                let retry_strategy = ExponentialBuilder::default().with_max_delay(*max_delay).with_min_delay(*min_delay).with_max_times(usize::from(*max_retries)).with_factor(factor.to_f32().unwrap_or(crate::config::retry::DEFAULT_RETRY_FACTOR)).with_jitter().build();
                (|| async { Dispatcher::dispatch_stream(&request_builder, req_body_bytes.clone(), api_endpoint.clone(), metrics_registry.clone()).await }).retry(retry_strategy).sleep(tokio::time::sleep).when(is_stream_retryable).notify(notify_stream_retry).await
            }
            RetryConfig::Constant { delay, max_retries } => {
                let retry_strategy = ConstantBuilder::default().with_delay(*delay).with_max_times(usize::from(*max_retries)).with_jitter().build();
                (|| async { Dispatcher::dispatch_stream(&request_builder, req_body_bytes.clone(), api_endpoint.clone(), metrics_registry.clone()).await }).retry(retry_strategy).sleep(tokio::time::sleep).when(is_stream_retryable).notify(notify_stream_retry).await
            }
        }
    } else { Dispatcher::dispatch_stream(&request_builder, req_body_bytes, api_endpoint, metrics_registry).await }
}

fn is_retryable_result(result: &Result<(http::Response<Body>, BodyReader, tokio::sync::oneshot::Receiver<()>), ApiError>) -> bool {
    match result {
        Ok(res) => res.0.status().is_server_error(),
        Err(ApiError::Internal(InternalError::ReqwestError(e))) => e.is_connect() || e.status().is_some_and(|s| s.is_server_error()),
        _ => false,
    }
}

fn notify_retry(result: &Result<(http::Response<Body>, BodyReader, tokio::sync::oneshot::Receiver<()>), ApiError>, dur: Duration) {
    if let Ok(res) = result { if res.0.status().is_server_error() { tracing::warn!(error = %res.0.status(), retry_in = ?dur, "retrying sync request..."); } }
    else if let Err(ApiError::Internal(InternalError::ReqwestError(e))) = result { tracing::warn!(error = %e, retry_in = ?dur, "retrying sync request..."); }
}

fn is_stream_retryable(e: &ApiError) -> bool { match e { ApiError::StreamError(s) => s.is_retryable(), _ => false } }

fn notify_stream_retry(err: &ApiError, dur: Duration) { tracing::warn!(error = %err, retry_in = ?dur, "retrying stream request..."); }

pub fn get_retry_config<'a>(app_state: &'a AppState, request_kind: RequestKind, req_ctx: &'a RequestContext) -> Option<&'a RetryConfig> {
    match request_kind {
        RequestKind::Router => router_config(app_state, req_ctx),
        RequestKind::UnifiedApi => app_state.config().unified_api.retries.as_ref(),
        RequestKind::DirectProxy => None,
    }
}

fn router_config<'a>(app_state: &'a AppState, req_ctx: &'a RequestContext) -> Option<&'a RetryConfig> {
    req_ctx.router_config.as_ref().and_then(|c| c.retries.as_ref()).or(app_state.config().global.retries.as_ref())
}

pub fn stream_response_headers() -> HeaderMap {
    HeaderMap::from_iter([
        (http::header::CONTENT_TYPE, HeaderValue::from_static("text/event-stream; charset=utf-8")),
        (http::header::CONNECTION, HeaderValue::from_static("keep-alive")),
        (http::header::TRANSFER_ENCODING, HeaderValue::from_static("chunked")),
    ])
}
