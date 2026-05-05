use std::time::Duration;

use backon::{BackoffBuilder, ConstantBuilder, ExponentialBuilder, Retryable};
use bytes::Bytes;
use http::{HeaderMap, HeaderValue, StatusCode};
use reqwest::RequestBuilder;
use rust_decimal::prelude::ToPrimitive;

use super::{Dispatcher, utils::extract_retry_after};
use crate::{
    app_state::AppState,
    config::retry::RetryConfig,
    discover::monitor::metrics::EndpointMetricsRegistry,
    endpoints::ApiEndpoint,
    error::{api::ApiError, internal::InternalError},
    types::{
        body::{Body, BodyReader},
        extensions::{RequestContext, RequestKind},
    },
};

/// Cap для Retry-After: если провайдер просит ждать дольше — выходим из
/// inline-retry, отдаём ответ как есть, дальнейший failover в роутере
/// сам решит уйти на cooldown и попробовать другую модель.
const RETRY_AFTER_CAP: Duration = Duration::from_secs(30);

impl Dispatcher {
    pub async fn dispatch_sync_with_retry(
        &self,
        request_builder: RequestBuilder,
        req_body_bytes: Bytes,
        req_ctx: &RequestContext,
        request_kind: RequestKind,
    ) -> Result<
        (
            http::Response<Body>,
            BodyReader,
            tokio::sync::oneshot::Receiver<()>,
        ),
        ApiError,
    > {
        let retry_config =
            get_retry_config(&self.app_state, request_kind, req_ctx);
        let Some(retry_config) = retry_config else {
            return Self::dispatch_sync(&request_builder, req_body_bytes).await;
        };

        // Кастомный retry-loop поверх backon-strategies: backon даёт нам
        // "сколько ждать по умолчанию", а мы сверху уважаем Retry-After
        // если ответ был 429 (или 503 с Retry-After). Это закрывает баг
        // что 429 раньше не ретраился вовсе и Retry-After игнорировался.
        let max_retries = retry_config.max_retries();
        let mut backoff_iter = backoff_iter_for(retry_config);
        let mut attempt: u16 = 0;
        loop {
            let result = Self::dispatch_sync(
                &request_builder,
                req_body_bytes.clone(),
            )
            .await;

            // Не-ретрай-достойный результат — отдаём как есть.
            if !is_retryable_result(&result) {
                return result;
            }

            attempt = attempt.saturating_add(1);
            if attempt > u16::from(max_retries) {
                return result;
            }

            // Retry-After из ответа (если 429 / 503 со встроенным таймаутом)
            // имеет приоритет над backon. Если он больше нашего cap'а —
            // дальнейший inline-retry бесполезен, отдаём результат и пусть
            // failover-логика роутера переходит на следующую модель.
            let retry_after = result
                .as_ref()
                .ok()
                .and_then(|r| retry_after_from_response(&r.0));
            let backoff_dur = backoff_iter
                .next()
                .unwrap_or_else(|| Duration::from_secs(1));
            let delay = match retry_after {
                Some(dur) if dur > RETRY_AFTER_CAP => {
                    tracing::warn!(
                        retry_after = ?dur,
                        cap = ?RETRY_AFTER_CAP,
                        "Retry-After exceeds cap — yielding to failover",
                    );
                    return result;
                }
                Some(dur) => dur.max(backoff_dur),
                None => backoff_dur,
            };
            notify_retry(&result, delay);
            tokio::time::sleep(delay).await;
        }
    }
}

/// Builds an iterator producing wait-durations per attempt.
fn backoff_iter_for(
    cfg: &RetryConfig,
) -> Box<dyn Iterator<Item = Duration> + Send> {
    match cfg {
        RetryConfig::Exponential {
            min_delay,
            max_delay,
            max_retries,
            factor,
        } => Box::new(
            ExponentialBuilder::default()
                .with_max_delay(*max_delay)
                .with_min_delay(*min_delay)
                .with_max_times(usize::from(*max_retries))
                .with_factor(factor.to_f32().unwrap_or(
                    crate::config::retry::DEFAULT_RETRY_FACTOR,
                ))
                .with_jitter()
                .build(),
        ),
        RetryConfig::Constant { delay, max_retries } => Box::new(
            ConstantBuilder::default()
                .with_delay(*delay)
                .with_max_times(usize::from(*max_retries))
                .with_jitter()
                .build(),
        ),
    }
}

trait RetryConfigExt {
    fn max_retries(&self) -> u8;
}

impl RetryConfigExt for RetryConfig {
    fn max_retries(&self) -> u8 {
        match self {
            RetryConfig::Exponential { max_retries, .. }
            | RetryConfig::Constant { max_retries, .. } => *max_retries,
        }
    }
}

fn retry_after_from_response(response: &http::Response<Body>) -> Option<Duration> {
    let status = response.status();
    if status != StatusCode::TOO_MANY_REQUESTS
        && status != StatusCode::SERVICE_UNAVAILABLE
    {
        return None;
    }
    extract_retry_after(response.headers()).map(Duration::from_secs)
}

pub async fn dispatch_stream_with_retry(
    app_state: &AppState,
    request_builder: RequestBuilder,
    req_body_bytes: Bytes,
    api_endpoint: Option<ApiEndpoint>,
    metrics_registry: EndpointMetricsRegistry,
    request_ctx: &RequestContext,
    request_kind: RequestKind,
) -> Result<
    (
        http::Response<Body>,
        BodyReader,
        tokio::sync::oneshot::Receiver<()>,
    ),
    ApiError,
> {
    let retry_config = get_retry_config(app_state, request_kind, request_ctx);
    if let Some(retry_config) = retry_config {
        match retry_config {
            RetryConfig::Exponential {
                min_delay,
                max_delay,
                max_retries,
                factor,
            } => {
                let retry_strategy =
                    ExponentialBuilder::default()
                        .with_max_delay(*max_delay)
                        .with_min_delay(*min_delay)
                        .with_max_times(usize::from(*max_retries))
                        .with_factor(factor.to_f32().unwrap_or(
                            crate::config::retry::DEFAULT_RETRY_FACTOR,
                        ))
                        .with_jitter()
                        .build();
                (|| async {
                    Dispatcher::dispatch_stream(
                        &request_builder,
                        req_body_bytes.clone(),
                        api_endpoint.clone(),
                        metrics_registry.clone(),
                    )
                    .await
                })
                .retry(retry_strategy)
                .sleep(tokio::time::sleep)
                .when(is_stream_retryable)
                .notify(notify_stream_retry)
                .await
            }
            RetryConfig::Constant { delay, max_retries } => {
                let retry_strategy = ConstantBuilder::default()
                    .with_delay(*delay)
                    .with_max_times(usize::from(*max_retries))
                    .with_jitter()
                    .build();
                (|| async {
                    Dispatcher::dispatch_stream(
                        &request_builder,
                        req_body_bytes.clone(),
                        api_endpoint.clone(),
                        metrics_registry.clone(),
                    )
                    .await
                })
                .retry(retry_strategy)
                .sleep(tokio::time::sleep)
                .when(is_stream_retryable)
                .notify(notify_stream_retry)
                .await
            }
        }
    } else {
        Dispatcher::dispatch_stream(
            &request_builder,
            req_body_bytes,
            api_endpoint,
            metrics_registry,
        )
        .await
    }
}

fn is_retryable_result(
    result: &Result<
        (
            http::Response<Body>,
            BodyReader,
            tokio::sync::oneshot::Receiver<()>,
        ),
        ApiError,
    >,
) -> bool {
    match result {
        Ok(res) => is_retryable_status(res.0.status()),
        Err(ApiError::Internal(InternalError::ReqwestError(e))) => {
            e.is_connect()
                || e.status().is_some_and(is_retryable_status)
        }
        _ => false,
    }
}

#[must_use]
pub fn is_retryable_status(status: StatusCode) -> bool {
    status == StatusCode::TOO_MANY_REQUESTS || status.is_server_error()
}

fn notify_retry(
    result: &Result<
        (
            http::Response<Body>,
            BodyReader,
            tokio::sync::oneshot::Receiver<()>,
        ),
        ApiError,
    >,
    dur: Duration,
) {
    if let Ok(res) = result {
        if res.0.status().is_server_error() {
            tracing::warn!(error = %res.0.status(), retry_in = ?dur, "retrying sync request...");
        }
    } else if let Err(ApiError::Internal(InternalError::ReqwestError(e))) =
        result
    {
        tracing::warn!(error = %e, retry_in = ?dur, "retrying sync request...");
    }
}

fn is_stream_retryable(e: &ApiError) -> bool {
    match e {
        ApiError::StreamError(s) => s.is_retryable(),
        _ => false,
    }
}

fn notify_stream_retry(err: &ApiError, dur: Duration) {
    tracing::warn!(error = %err, retry_in = ?dur, "retrying stream request...");
}

pub fn get_retry_config<'a>(
    app_state: &'a AppState,
    request_kind: RequestKind,
    req_ctx: &'a RequestContext,
) -> Option<&'a RetryConfig> {
    match request_kind {
        RequestKind::Router => router_config(app_state, req_ctx),
        RequestKind::UnifiedApi => {
            app_state.config().unified_api.retries.as_ref()
        }
        RequestKind::DirectProxy => None,
    }
}

fn router_config<'a>(
    app_state: &'a AppState,
    req_ctx: &'a RequestContext,
) -> Option<&'a RetryConfig> {
    req_ctx
        .router_config
        .as_ref()
        .and_then(|c| c.retries.as_ref())
        .or(app_state.config().global.retries.as_ref())
}

pub fn stream_response_headers() -> HeaderMap {
    HeaderMap::from_iter([
        (
            http::header::CONTENT_TYPE,
            HeaderValue::from_static("text/event-stream; charset=utf-8"),
        ),
        (
            http::header::CONNECTION,
            HeaderValue::from_static("keep-alive"),
        ),
        (
            http::header::TRANSFER_ENCODING,
            HeaderValue::from_static("chunked"),
        ),
    ])
}

#[cfg(test)]
mod retry_tests {
    use super::*;

    fn resp_with(
        status: StatusCode,
        retry_after: Option<&'static str>,
    ) -> http::Response<Body> {
        let mut builder = http::Response::builder().status(status);
        if let Some(ra) = retry_after {
            builder = builder.header(http::header::RETRY_AFTER, ra);
        }
        builder.body(Body::empty()).unwrap()
    }

    #[test]
    fn retryable_status_includes_429_and_5xx() {
        assert!(is_retryable_status(StatusCode::TOO_MANY_REQUESTS));
        assert!(is_retryable_status(StatusCode::INTERNAL_SERVER_ERROR));
        assert!(is_retryable_status(StatusCode::BAD_GATEWAY));
        assert!(is_retryable_status(StatusCode::SERVICE_UNAVAILABLE));
        assert!(is_retryable_status(StatusCode::GATEWAY_TIMEOUT));
    }

    #[test]
    fn retryable_status_excludes_4xx_and_2xx() {
        assert!(!is_retryable_status(StatusCode::OK));
        assert!(!is_retryable_status(StatusCode::BAD_REQUEST));
        assert!(!is_retryable_status(StatusCode::UNAUTHORIZED));
        assert!(!is_retryable_status(StatusCode::NOT_FOUND));
    }

    #[test]
    fn retry_after_extracted_from_429() {
        let r = resp_with(StatusCode::TOO_MANY_REQUESTS, Some("12"));
        assert_eq!(
            retry_after_from_response(&r),
            Some(Duration::from_secs(12))
        );
    }

    #[test]
    fn retry_after_extracted_from_503() {
        let r = resp_with(StatusCode::SERVICE_UNAVAILABLE, Some("3"));
        assert_eq!(
            retry_after_from_response(&r),
            Some(Duration::from_secs(3))
        );
    }

    #[test]
    fn retry_after_ignored_on_200() {
        // Even with header present, we don't treat 200 as needing wait.
        let r = resp_with(StatusCode::OK, Some("99"));
        assert_eq!(retry_after_from_response(&r), None);
    }

    #[test]
    fn retry_after_ignored_on_500() {
        // 500 is retryable but doesn't carry meaningful Retry-After
        // semantics in our model; backon's strategy applies.
        let r = resp_with(StatusCode::INTERNAL_SERVER_ERROR, Some("5"));
        assert_eq!(retry_after_from_response(&r), None);
    }

    #[test]
    fn retry_after_missing_header_returns_none() {
        let r = resp_with(StatusCode::TOO_MANY_REQUESTS, None);
        assert_eq!(retry_after_from_response(&r), None);
    }
}
