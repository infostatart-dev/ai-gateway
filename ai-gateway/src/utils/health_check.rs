use std::{
    future::{Ready, ready},
    marker::PhantomData,
    pin::Pin,
    task::{Context, Poll},
};

use axum_core::response::Response;
use chrono::{DateTime, Utc};
use futures::future::Either;
use http::{Method, Request, StatusCode, header::CONTENT_TYPE};
use tower::{Layer, Service};

use crate::app_state::AppState;

type BoxFuture<T> = Pin<Box<dyn std::future::Future<Output = T> + Send>>;

#[derive(Debug, Clone)]
pub struct HealthCheckLayer<ReqBody, E> {
    app_state: AppState,
    _marker: PhantomData<(ReqBody, E)>,
}

impl<ReqBody, E> HealthCheckLayer<ReqBody, E> {
    #[must_use]
    pub fn new(app_state: AppState) -> Self {
        Self {
            app_state,
            _marker: PhantomData,
        }
    }
}

impl<S, ReqBody, E> Layer<S> for HealthCheckLayer<ReqBody, E>
where
    S: tower::Service<http::Request<ReqBody>, Response = Response, Error = E>,
{
    type Service = HealthCheck<S, ReqBody, E>;

    fn layer(&self, inner: S) -> Self::Service {
        HealthCheck::new(self.app_state.clone(), inner)
    }
}

#[derive(Debug)]
pub struct HealthCheck<S, ReqBody, E> {
    app_state: AppState,
    inner: S,
    _marker: PhantomData<(ReqBody, E)>,
}

impl<S: Clone, ReqBody, E> Clone for HealthCheck<S, ReqBody, E> {
    fn clone(&self) -> Self {
        Self {
            app_state: self.app_state.clone(),
            inner: self.inner.clone(),
            _marker: PhantomData,
        }
    }
}

impl<S, ReqBody, E> HealthCheck<S, ReqBody, E>
where
    S: tower::Service<http::Request<ReqBody>, Response = Response, Error = E>,
{
    pub fn new(app_state: AppState, inner: S) -> Self {
        Self {
            app_state,
            inner,
            _marker: PhantomData,
        }
    }
}

impl<S, ReqBody, E> Service<Request<ReqBody>> for HealthCheck<S, ReqBody, E>
where
    S: Service<Request<ReqBody>, Response = Response, Error = E>
        + Send
        + 'static,
    S::Future: Send + 'static,
    ReqBody: Send + 'static,
{
    type Response = Response;
    type Error = S::Error;
    type Future = Either<
        Either<
            Ready<Result<Self::Response, Self::Error>>,
            BoxFuture<Result<Self::Response, Self::Error>>,
        >,
        S::Future,
    >;

    fn poll_ready(
        &mut self,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request<ReqBody>) -> Self::Future {
        if req.method() == Method::GET || req.method() == Method::HEAD {
            let path = req.uri().path().to_string();
            if path == "/health" {
                return Either::Left(Either::Left(ready(Ok(
                    healthy_response(&self.app_state),
                ))));
            }
            if path == "/metrics" {
                return Either::Left(Either::Left(ready(Ok(
                    metrics_response(),
                ))));
            }
            if path == "/models" {
                return Either::Left(Either::Left(ready(Ok(
                    declared_models_response(),
                ))));
            }
            if let Some((provider, credential)) = observability_filter(&path) {
                let app_state = self.app_state.clone();
                return Either::Left(Either::Right(Box::pin(async move {
                    let snapshot = app_state
                        .provider_stats_snapshot_async(
                            provider.as_deref(),
                            credential.as_deref(),
                        )
                        .await;
                    Ok(json_response(snapshot))
                })));
            }
        }
        Either::Right(self.inner.call(req))
    }
}

#[derive(Debug, serde::Serialize)]
struct HealthSnapshot {
    version: &'static str,
    started_at_utc: DateTime<Utc>,
    started_at_server_time: String,
    started_at_server_timezone: String,
}

fn healthy_response(app_state: &AppState) -> Response {
    let snapshot = app_state.provider_stats_snapshot(None, None);
    json_response(HealthSnapshot {
        version: snapshot.version,
        started_at_utc: snapshot.started_at_utc,
        started_at_server_time: snapshot.started_at_server_time,
        started_at_server_timezone: snapshot.started_at_server_timezone,
    })
}

#[derive(Debug, serde::Serialize)]
struct MetricsErrorSnapshot {
    error: String,
}

fn metrics_response() -> Response {
    match telemetry::prometheus_metrics_text() {
        Some(Ok(metrics)) => http::Response::builder()
            .status(StatusCode::OK)
            .header(CONTENT_TYPE, metrics.content_type)
            .body(axum_core::body::Body::from(metrics.body))
            .expect("valid metrics response"),
        Some(Err(error)) => json_status_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            MetricsErrorSnapshot { error },
        ),
        None => json_status_response(
            StatusCode::SERVICE_UNAVAILABLE,
            MetricsErrorSnapshot {
                error: "metrics exporter not initialized".to_string(),
            },
        ),
    }
}

fn declared_models_response() -> Response {
    json_response(crate::declared_models::catalog())
}

fn observability_filter(
    path: &str,
) -> Option<(Option<String>, Option<String>)> {
    const PREFIX: &str = "/v1/observability/provider-stats";
    if path == PREFIX {
        return Some((None, None));
    }
    let rest = path.strip_prefix(PREFIX)?.strip_prefix('/')?;
    if rest.is_empty() || rest.contains('/') {
        return None;
    }
    Some((Some(rest.to_string()), None))
}

fn json_response<T: serde::Serialize>(value: T) -> Response {
    json_status_response(StatusCode::OK, value)
}

fn json_status_response<T: serde::Serialize>(
    status: StatusCode,
    value: T,
) -> Response {
    match serde_json::to_vec(&value) {
        Ok(body) => http::Response::builder()
            .status(status)
            .header(CONTENT_TYPE, "application/json")
            .body(axum_core::body::Body::from(body))
            .expect("valid json response"),
        Err(_) => http::Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .body(axum_core::body::Body::empty())
            .expect("valid error response"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn observability_filter_accepts_top_level_stats() {
        assert_eq!(
            observability_filter("/v1/observability/provider-stats"),
            Some((None, None))
        );
    }
}
