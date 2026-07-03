use std::{
    future::Future,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};

use axum_core::body::Body;
use futures::StreamExt;
use http_body_util::{BodyExt, Limited};
use opentelemetry::KeyValue;
use tower::{Layer, Service};

use crate::{
    app_state::AppState,
    client_access::quota::{
        QuotaAdmission, QuotaAdmissionError, QuotaDimension, QuotaFamily,
        QuotaLimitStatus, QuotaRejection, QuotaReservation, QuotaWindowKind,
    },
    endpoints::{ApiEndpoint, EndpointType},
    error::{
        api::ApiError,
        internal::InternalError,
        invalid_req::{InvalidRequestError, TooManyRequestsError},
    },
    router::token_estimate::{PayloadBudgetConfig, estimate_from_value},
    types::{
        extensions::{
            ClientAccessContext, GatewayProviderUsageExtension, MapperContext,
        },
        request::Request,
        response::Response,
    },
};

#[derive(Debug, Clone)]
pub struct ClientAccessQuotaLayer {
    app_state: AppState,
}

impl ClientAccessQuotaLayer {
    #[must_use]
    pub fn new(app_state: AppState) -> Self {
        Self { app_state }
    }
}

impl<S> Layer<S> for ClientAccessQuotaLayer {
    type Service = ClientAccessQuotaService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        ClientAccessQuotaService {
            inner,
            app_state: self.app_state.clone(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ClientAccessQuotaService<S> {
    inner: S,
    app_state: AppState,
}

impl<S> Service<Request> for ClientAccessQuotaService<S>
where
    S: Service<Request, Response = Response, Error = ApiError>
        + Clone
        + Send
        + 'static,
    S::Future: Send + 'static,
{
    type Response = Response;
    type Error = ApiError;
    type Future = Pin<
        Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>,
    >;

    fn poll_ready(
        &mut self,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request) -> Self::Future {
        let mut inner = self.inner.clone();
        std::mem::swap(&mut self.inner, &mut inner);
        let app_state = self.app_state.clone();
        Box::pin(async move {
            let Some(client_ctx) =
                req.extensions().get::<ClientAccessContext>().cloned()
            else {
                return inner.call(req).await;
            };
            let Some(store) = app_state.client_access_quota_store() else {
                return inner.call(req).await;
            };

            let now = chrono::Utc::now();
            let max_body_bytes =
                app_state.config().client_access.max_body_bytes;
            let estimate_tokens = is_chat_request(&req);
            let (parts, body) = req.into_parts();
            let body_bytes = Limited::new(body, max_body_bytes)
                .collect()
                .await
                .map_err(|_| {
                    ApiError::InvalidRequest(
                        InvalidRequestError::BudgetExceeded(format!(
                            "request body exceeds \
                             client-access.max-body-bytes of {max_body_bytes}",
                        )),
                    )
                })?
                .to_bytes();

            let reservation = reserve_client_access_tokens(
                &app_state,
                &store,
                &client_ctx,
                &body_bytes,
                estimate_tokens,
                now,
            )
            .await?;

            let request_admission = match admit_client_access_request(
                &app_state,
                &store,
                &client_ctx,
                now,
            )
            .await
            {
                Ok(admission) => admission,
                Err(error) => {
                    refund_reservation(&store, reservation.as_ref()).await;
                    record_quota_error(&app_state, &client_ctx, &error);
                    return Err(quota_admission_error(error));
                }
            };

            let req = Request::from_parts(parts, Body::from(body_bytes));
            match inner.call(req).await {
                Ok(mut response) => {
                    insert_success_rate_limit_headers(
                        response.headers_mut(),
                        &request_admission,
                        reservation.as_ref(),
                    );
                    if let Some(reservation) = reservation {
                        if is_streaming_response(&response) {
                            return Ok(wrap_streaming_settlement(
                                response,
                                store,
                                reservation,
                            ));
                        }
                        let final_amount = reported_total_tokens(&response)
                            .unwrap_or(reservation.amount);
                        store
                            .commit_tokens(
                                &reservation,
                                final_amount,
                                chrono::Utc::now(),
                            )
                            .await
                            .map_err(|error| quota_store_error(&error))?;
                    }
                    Ok(response)
                }
                Err(error) => {
                    if let Some(reservation) = reservation {
                        let _ = store
                            .refund_tokens(&reservation, chrono::Utc::now())
                            .await;
                    }
                    Err(error)
                }
            }
        })
    }
}

async fn reserve_client_access_tokens(
    app_state: &AppState,
    store: &Arc<dyn crate::client_access::quota::ClientAccessQuotaStore>,
    client_ctx: &ClientAccessContext,
    body_bytes: &bytes::Bytes,
    estimate_tokens: bool,
    now: chrono::DateTime<chrono::Utc>,
) -> Result<Option<QuotaReservation>, ApiError> {
    if !estimate_tokens {
        return Ok(None);
    }
    let total_tokens = estimate_client_access_tokens(body_bytes, client_ctx);
    if total_tokens == 0 {
        return Ok(None);
    }
    match store
        .reserve_tokens(
            &client_ctx.key_id,
            u64::from(total_tokens),
            &client_ctx.quota_limits.tokens,
            now,
        )
        .await
    {
        Ok(reservation) => {
            record_quota_admission(app_state, client_ctx, "tokens");
            Ok(Some(reservation))
        }
        Err(error) => {
            record_quota_error(app_state, client_ctx, &error);
            Err(quota_admission_error(error))
        }
    }
}

async fn admit_client_access_request(
    app_state: &AppState,
    store: &Arc<dyn crate::client_access::quota::ClientAccessQuotaStore>,
    client_ctx: &ClientAccessContext,
    now: chrono::DateTime<chrono::Utc>,
) -> Result<QuotaAdmission, QuotaAdmissionError> {
    let admission = store
        .admit_request(
            &client_ctx.key_id,
            &client_ctx.quota_limits.requests,
            now,
        )
        .await?;
    record_quota_admission(app_state, client_ctx, "requests");
    Ok(admission)
}

fn record_quota_admission(
    app_state: &AppState,
    client_ctx: &ClientAccessContext,
    family: &'static str,
) {
    app_state.0.metrics.client_access.quota_admissions.add(
        1,
        &[
            KeyValue::new("family", family),
            KeyValue::new("key_id", client_ctx.key_id.clone()),
            KeyValue::new("plan_id", client_ctx.plan_id.clone()),
        ],
    );
}

async fn refund_reservation(
    store: &Arc<dyn crate::client_access::quota::ClientAccessQuotaStore>,
    reservation: Option<&QuotaReservation>,
) {
    if let Some(reservation) = reservation {
        let _ = store.refund_tokens(reservation, chrono::Utc::now()).await;
    }
}

fn is_streaming_response(response: &Response) -> bool {
    response
        .extensions()
        .get::<MapperContext>()
        .is_some_and(|ctx| ctx.is_stream)
}

struct StreamState {
    stream: Pin<
        Box<
            dyn futures::Stream<Item = Result<bytes::Bytes, axum_core::Error>>
                + Send,
        >,
    >,
    guard: StreamingSettlementGuard,
    done: bool,
}

struct StreamingSettlementGuard {
    store: Arc<dyn crate::client_access::quota::ClientAccessQuotaStore>,
    reservation: Option<QuotaReservation>,
    reported_total: Option<u64>,
}

impl Drop for StreamingSettlementGuard {
    fn drop(&mut self) {
        let Some(reservation) = self.reservation.take() else {
            return;
        };
        let store = Arc::clone(&self.store);
        let reported_total = self.reported_total;
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            handle.spawn(async move {
                if let Some(total) = reported_total {
                    let _ = store
                        .commit_tokens(&reservation, total, chrono::Utc::now())
                        .await;
                } else {
                    let _ = store
                        .refund_tokens(&reservation, chrono::Utc::now())
                        .await;
                }
            });
        }
    }
}

impl StreamingSettlementGuard {
    async fn commit(&mut self, amount: u64) {
        if let Some(reservation) = self.reservation.as_ref() {
            let _ = self
                .store
                .commit_tokens(reservation, amount, chrono::Utc::now())
                .await;
            self.reservation = None;
        }
    }

    async fn refund(&mut self) {
        if let Some(reservation) = self.reservation.as_ref() {
            let _ = self
                .store
                .refund_tokens(reservation, chrono::Utc::now())
                .await;
            self.reservation = None;
        }
    }

    async fn commit_reported_or_reserved(&mut self) {
        let final_amount = self.reported_total.unwrap_or_else(|| {
            self.reservation
                .as_ref()
                .map_or(0, |reservation| reservation.amount)
        });
        self.commit(final_amount).await;
    }
}

fn wrap_streaming_settlement(
    response: Response,
    store: Arc<dyn crate::client_access::quota::ClientAccessQuotaStore>,
    reservation: QuotaReservation,
) -> Response {
    let (parts, body) = response.into_parts();
    let state = StreamState {
        stream: Box::pin(body.into_data_stream()),
        guard: StreamingSettlementGuard {
            store,
            reservation: Some(reservation),
            reported_total: None,
        },
        done: false,
    };
    let stream = futures::stream::unfold(state, |mut state| async move {
        if state.done {
            return None;
        }
        match state.stream.next().await {
            Some(Ok(chunk)) => {
                if let Some(total) = reported_stream_total_tokens(&chunk) {
                    state.guard.reported_total = Some(total);
                }
                Some((Ok(chunk), state))
            }
            Some(Err(error)) => {
                if let Some(total) = state.guard.reported_total {
                    state.guard.commit(total).await;
                } else {
                    state.guard.refund().await;
                }
                state.done = true;
                Some((Err(error), state))
            }
            None => {
                state.guard.commit_reported_or_reserved().await;
                None
            }
        }
    })
    .fuse();
    Response::from_parts(parts, Body::from_stream(stream))
}

fn reported_stream_total_tokens(chunk: &bytes::Bytes) -> Option<u64> {
    let text = std::str::from_utf8(chunk).ok()?;
    text.lines()
        .filter_map(|line| line.trim().strip_prefix("data:").map(str::trim))
        .filter(|payload| !payload.is_empty() && *payload != "[DONE]")
        .filter_map(|payload| {
            serde_json::from_str::<serde_json::Value>(payload).ok()
        })
        .find_map(|value| {
            value
                .pointer("/usage/total_tokens")
                .or_else(|| value.pointer("/usage/total"))
                .and_then(serde_json::Value::as_u64)
        })
}

fn reported_total_tokens(response: &Response) -> Option<u64> {
    let usage = response
        .extensions()
        .get::<GatewayProviderUsageExtension>()?
        .0
        .usage
        .clone();
    if usage.source == "reported" {
        usage.total
    } else {
        None
    }
}

fn insert_success_rate_limit_headers(
    headers: &mut http::HeaderMap,
    request_admission: &QuotaAdmission,
    reservation: Option<&QuotaReservation>,
) {
    if let Some(status) = request_admission.most_constrained.as_ref() {
        insert_limit_headers(headers, "", status);
    }
    if let Some(status) =
        reservation.and_then(|r| r.admission.most_constrained.as_ref())
    {
        insert_limit_headers(headers, "token-", status);
    }
}

fn insert_limit_headers(
    headers: &mut http::HeaderMap,
    prefix: &'static str,
    status: &QuotaLimitStatus,
) {
    let limit_name = if prefix.is_empty() {
        http::header::HeaderName::from_static("x-ratelimit-limit")
    } else {
        http::header::HeaderName::from_static("x-token-ratelimit-limit")
    };
    let remaining_name = if prefix.is_empty() {
        http::header::HeaderName::from_static("x-ratelimit-remaining")
    } else {
        http::header::HeaderName::from_static("x-token-ratelimit-remaining")
    };
    let dimension_name = if prefix.is_empty() {
        http::header::HeaderName::from_static("x-ratelimit-dimension")
    } else {
        http::header::HeaderName::from_static("x-token-ratelimit-dimension")
    };
    if let Ok(value) = status.limit.to_string().parse() {
        headers.insert(limit_name, value);
    }
    if let Ok(value) = status.remaining.to_string().parse() {
        headers.insert(remaining_name, value);
    }
    headers.insert(
        dimension_name,
        http::HeaderValue::from_static(quota_dimension_label(status.dimension)),
    );
}

fn quota_dimension_label(dimension: QuotaDimension) -> &'static str {
    match (dimension.family, dimension.window) {
        (QuotaFamily::Requests, QuotaWindowKind::Minute) => {
            "requests.per-minute"
        }
        (QuotaFamily::Requests, QuotaWindowKind::Day) => "requests.per-day",
        (QuotaFamily::Requests, QuotaWindowKind::Week) => "requests.per-week",
        (QuotaFamily::Tokens, QuotaWindowKind::Minute) => "tokens.per-minute",
        (QuotaFamily::Tokens, QuotaWindowKind::Day) => "tokens.per-day",
        (QuotaFamily::Tokens, QuotaWindowKind::Week) => "tokens.per-week",
    }
}

fn is_chat_request(req: &Request) -> bool {
    req.extensions()
        .get::<ApiEndpoint>()
        .is_none_or(|endpoint| endpoint.endpoint_type() == EndpointType::Chat)
}

fn estimate_client_access_tokens(
    body: &bytes::Bytes,
    client_ctx: &ClientAccessContext,
) -> u32 {
    let Ok(value) = serde_json::from_slice(body) else {
        return client_ctx.max_output_tokens;
    };
    let config = PayloadBudgetConfig {
        default_output_tokens: client_ctx.max_output_tokens,
        safety_margin_pct: 0,
    };
    estimate_from_value(&value, config).map_or(
        client_ctx.max_output_tokens,
        crate::router::token_estimate::PayloadEstimate::total,
    )
}

fn quota_admission_error(error: QuotaAdmissionError) -> ApiError {
    match error {
        QuotaAdmissionError::Rejected(rejection) => quota_rejection(&rejection),
        QuotaAdmissionError::Store(error) => quota_store_error(&error),
    }
}

fn record_quota_error(
    app_state: &AppState,
    client_ctx: &ClientAccessContext,
    error: &QuotaAdmissionError,
) {
    match error {
        QuotaAdmissionError::Rejected(rejection) => {
            let dimension = format!("{:?}", rejection.dimension);
            app_state.0.metrics.client_access.quota_rejections.add(
                1,
                &[
                    KeyValue::new("dimension", dimension.clone()),
                    KeyValue::new("key_id", client_ctx.key_id.clone()),
                    KeyValue::new("plan_id", client_ctx.plan_id.clone()),
                ],
            );
            tracing::warn!(
                key_id = %client_ctx.key_id,
                plan_id = %client_ctx.plan_id,
                dimension = %dimension,
                "client access quota rejected request",
            );
        }
        QuotaAdmissionError::Store(error) => {
            app_state.0.metrics.client_access.quota_store_errors.add(
                1,
                &[
                    KeyValue::new("key_id", client_ctx.key_id.clone()),
                    KeyValue::new("plan_id", client_ctx.plan_id.clone()),
                ],
            );
            tracing::warn!(
                key_id = %client_ctx.key_id,
                plan_id = %client_ctx.plan_id,
                error = %error,
                "client access quota store failed closed",
            );
        }
    }
}

fn quota_rejection(rejection: &QuotaRejection) -> ApiError {
    ApiError::InvalidRequest(InvalidRequestError::TooManyRequests(
        TooManyRequestsError {
            ratelimit_limit: rejection.limit,
            ratelimit_remaining: rejection.remaining(),
            retry_after: rejection.retry_after_seconds,
        },
    ))
}

fn quota_store_error(
    error: &crate::client_access::quota::QuotaStoreError,
) -> ApiError {
    ApiError::Internal(InternalError::ClientAccessQuotaUnavailable(
        error.to_string(),
    ))
}

#[cfg(all(test, feature = "testing"))]
mod tests {
    use std::{
        sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        },
        task::{Context, Poll},
    };

    use bytes::Bytes;
    use chrono::Utc;
    use futures::stream;
    use http::{Request as HttpRequest, Response as HttpResponse, StatusCode};
    use http_body_util::BodyExt;
    use tower::Service;
    use uuid::Uuid;

    use super::*;
    use crate::{
        app::App,
        client_access::quota::{
            MemoryClientAccessQuotaStore, store::ClientAccessQuotaStore,
        },
        config::{
            Config,
            client_access::{
                ClientAccessConfig, ClientAccessLimitsConfig,
                ClientAccessQuotaStoreConfig, ClientAccessWindowLimitsConfig,
            },
        },
        metrics::provider::{
            GatewayProviderUsage,
            usage_json::{LatencyBlock, RoutingBlock, UsageBlock},
        },
        tests::TestDefault,
        types::{org::OrgId, user::UserId},
    };

    #[derive(Clone)]
    struct ResponseService {
        calls: Arc<AtomicUsize>,
        reported_total: Option<u64>,
    }

    impl Service<Request> for ResponseService {
        type Response = Response;
        type Error = ApiError;
        type Future = futures::future::Ready<Result<Response, ApiError>>;

        fn poll_ready(
            &mut self,
            _cx: &mut Context<'_>,
        ) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }

        fn call(&mut self, _req: Request) -> Self::Future {
            self.calls.fetch_add(1, Ordering::SeqCst);
            let mut response = HttpResponse::builder()
                .status(StatusCode::OK)
                .body(Body::from("{}"))
                .unwrap();
            if let Some(total) = self.reported_total {
                response.extensions_mut().insert(
                    GatewayProviderUsageExtension(GatewayProviderUsage {
                        provider: "openai".to_string(),
                        credential: None,
                        model: Some("gpt-4o-mini".to_string()),
                        usage: UsageBlock {
                            input: None,
                            output: None,
                            cached: None,
                            reasoning: None,
                            total: Some(total),
                            source: "reported",
                        },
                        latency_ms: LatencyBlock {
                            total: 1.0,
                            ttfb: None,
                            ttft: None,
                            generation_per_output_token: None,
                        },
                        routing: RoutingBlock {
                            attempts: 1,
                            failover: false,
                        },
                    }),
                );
            }
            futures::future::ready(Ok(response))
        }
    }

    async fn app_state(max_body_bytes: usize) -> AppState {
        let path = std::env::temp_dir().join(format!(
            "ai-gateway-client-access-quota-{}-{}.yaml",
            std::process::id(),
            Uuid::new_v4()
        ));
        std::fs::write(
            &path,
            format!(
                r#"
version: 1
subjects:
  test:
    org-id: "00000000-0000-0000-0000-000000000001"
    user-id: "00000000-0000-0000-0000-000000000002"
plans:
  starter:
    limits:
      requests:
        per-minute: 100
      tokens:
        per-minute: 100000
keys:
  test-key:
    hash: "{}"
    subject: test
    status: active
    plan: starter
    scopes:
      - "*"
"#,
                crate::client_access::ClientAccessKeyHash::from_bearer_token(
                    "sk-test"
                )
            ),
        )
        .unwrap();
        let mut config = Config::test_default();
        config.client_access = ClientAccessConfig {
            enabled: true,
            file: Some(path),
            reload_interval: std::time::Duration::from_secs(1),
            max_body_bytes,
            quota_store: ClientAccessQuotaStoreConfig::Memory,
        };
        App::new(config).await.unwrap().state
    }

    fn client_ctx(
        key_id: &str,
        request_limit: u64,
        token_limit: u64,
    ) -> ClientAccessContext {
        ClientAccessContext {
            key_id: key_id.to_string(),
            subject_id: "subject".to_string(),
            user_id: UserId::new(Uuid::nil()),
            org_id: OrgId::new(Uuid::nil()),
            plan_id: "starter".to_string(),
            max_output_tokens: 100,
            scopes: vec![crate::client_access::ClientAccessScope::All],
            quota_limits: ClientAccessLimitsConfig {
                requests: ClientAccessWindowLimitsConfig {
                    per_minute: Some(request_limit),
                    per_day: None,
                    per_week: None,
                },
                tokens: ClientAccessWindowLimitsConfig {
                    per_minute: Some(token_limit),
                    per_day: None,
                    per_week: None,
                },
            },
        }
    }

    fn quota_request(
        body: impl Into<Body>,
        ctx: ClientAccessContext,
    ) -> Request {
        let mut req = HttpRequest::builder()
            .uri("http://gateway.local/ai/chat/completions")
            .body(body.into())
            .unwrap();
        req.extensions_mut().insert(ctx);
        req
    }

    #[tokio::test]
    async fn client_access_body_size_rejection_does_not_consume_request_quota()
    {
        let app_state = app_state(8).await;
        let calls = Arc::new(AtomicUsize::new(0));
        let mut service =
            ClientAccessQuotaLayer::new(app_state).layer(ResponseService {
                calls: calls.clone(),
                reported_total: None,
            });
        let ctx = client_ctx("body-limit-key", 1, 1000);

        let error = service
            .call(quota_request("0123456789", ctx.clone()))
            .await
            .unwrap_err();
        assert!(matches!(
            error,
            ApiError::InvalidRequest(InvalidRequestError::BudgetExceeded(_))
        ));
        assert_eq!(calls.load(Ordering::SeqCst), 0);

        let response = service
            .call(quota_request("{}", ctx))
            .await
            .expect("second request should use unconsumed request quota");
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn client_access_token_rejection_does_not_consume_request_quota() {
        let app_state = app_state(1024).await;
        let calls = Arc::new(AtomicUsize::new(0));
        let mut service =
            ClientAccessQuotaLayer::new(app_state).layer(ResponseService {
                calls: calls.clone(),
                reported_total: None,
            });

        let rejected = service
            .call(quota_request(
                r#"{"messages":[],"max_tokens":100}"#,
                client_ctx("token-reject-key", 1, 1),
            ))
            .await
            .unwrap_err();
        assert!(matches!(
            rejected,
            ApiError::InvalidRequest(InvalidRequestError::TooManyRequests(_))
        ));
        assert_eq!(calls.load(Ordering::SeqCst), 0);

        let response = service
            .call(quota_request(
                r#"{"messages":[],"max_tokens":10}"#,
                client_ctx("token-reject-key", 1, 1000),
            ))
            .await
            .expect("request quota should not be consumed by token rejection");
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn client_access_success_response_includes_limit_headers() {
        let app_state = app_state(1024).await;
        let mut service =
            ClientAccessQuotaLayer::new(app_state).layer(ResponseService {
                calls: Arc::new(AtomicUsize::new(0)),
                reported_total: None,
            });
        let response = service
            .call(quota_request(
                r#"{"messages":[],"max_tokens":10}"#,
                client_ctx("header-key", 1, 1000),
            ))
            .await
            .unwrap();

        assert_eq!(response.headers().get("x-ratelimit-limit").unwrap(), "1");
        assert_eq!(
            response.headers().get("x-ratelimit-remaining").unwrap(),
            "0"
        );
        assert_eq!(
            response.headers().get("x-ratelimit-dimension").unwrap(),
            "requests.per-minute"
        );
        assert!(response.headers().contains_key("x-token-ratelimit-limit"));
    }

    #[tokio::test]
    async fn client_access_non_streaming_commits_reported_usage() {
        let app_state = app_state(1024).await;
        let store = app_state.client_access_quota_store().unwrap();
        let mut service =
            ClientAccessQuotaLayer::new(app_state).layer(ResponseService {
                calls: Arc::new(AtomicUsize::new(0)),
                reported_total: Some(60),
            });
        let ctx = client_ctx("reported-key", 100, 200);

        let response = service
            .call(quota_request(
                r#"{"messages":[],"max_tokens":100}"#,
                ctx.clone(),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        assert!(
            store
                .reserve_tokens(
                    &ctx.key_id,
                    140,
                    &ctx.quota_limits.tokens,
                    Utc::now(),
                )
                .await
                .is_ok()
        );
    }

    #[tokio::test]
    async fn client_access_streaming_success_commits_reported_usage() {
        let store = Arc::new(MemoryClientAccessQuotaStore::new());
        let limits = ClientAccessWindowLimitsConfig {
            per_minute: Some(130),
            per_day: None,
            per_week: None,
        };
        let reservation = store
            .reserve_tokens("stream-key", 100, &limits, Utc::now())
            .await
            .unwrap();
        let body =
            Body::from_stream(stream::iter([Ok::<Bytes, axum_core::Error>(
                Bytes::from_static(
                    br#"data: {"choices":[],"usage":{"total_tokens":60}}

"#,
                ),
            )]));
        let response = wrap_streaming_settlement(
            HttpResponse::builder().body(body).unwrap(),
            store.clone(),
            reservation,
        );

        response.into_body().collect().await.unwrap();

        assert!(
            store
                .reserve_tokens("stream-key", 70, &limits, Utc::now())
                .await
                .is_ok()
        );
    }

    #[tokio::test]
    async fn client_access_streaming_error_before_usage_refunds() {
        let store = Arc::new(MemoryClientAccessQuotaStore::new());
        let limits = ClientAccessWindowLimitsConfig {
            per_minute: Some(100),
            per_day: None,
            per_week: None,
        };
        let reservation = store
            .reserve_tokens("stream-error-key", 100, &limits, Utc::now())
            .await
            .unwrap();
        let body =
            Body::from_stream(stream::iter([Err::<Bytes, axum_core::Error>(
                axum_core::Error::new(std::io::Error::other("boom")),
            )]));
        let response = wrap_streaming_settlement(
            HttpResponse::builder().body(body).unwrap(),
            store.clone(),
            reservation,
        );

        assert!(response.into_body().collect().await.is_err());
        assert!(
            store
                .reserve_tokens("stream-error-key", 100, &limits, Utc::now())
                .await
                .is_ok()
        );
    }

    #[tokio::test]
    async fn client_access_streaming_error_after_usage_commits_reported_usage()
    {
        let store = Arc::new(MemoryClientAccessQuotaStore::new());
        let limits = ClientAccessWindowLimitsConfig {
            per_minute: Some(130),
            per_day: None,
            per_week: None,
        };
        let reservation = store
            .reserve_tokens(
                "stream-error-after-usage-key",
                100,
                &limits,
                Utc::now(),
            )
            .await
            .unwrap();
        let body = Body::from_stream(stream::iter([
            Ok::<Bytes, axum_core::Error>(Bytes::from_static(
                br#"data: {"choices":[],"usage":{"total_tokens":60}}

"#,
            )),
            Err(axum_core::Error::new(std::io::Error::other("boom"))),
        ]));
        let response = wrap_streaming_settlement(
            HttpResponse::builder().body(body).unwrap(),
            store.clone(),
            reservation,
        );

        assert!(response.into_body().collect().await.is_err());
        assert!(
            store
                .reserve_tokens(
                    "stream-error-after-usage-key",
                    70,
                    &limits,
                    Utc::now()
                )
                .await
                .is_ok()
        );
        assert!(
            store
                .reserve_tokens(
                    "stream-error-after-usage-key",
                    1,
                    &limits,
                    Utc::now()
                )
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn client_access_streaming_body_drop_refunds_unsettled_reservation() {
        let store = Arc::new(MemoryClientAccessQuotaStore::new());
        let limits = ClientAccessWindowLimitsConfig {
            per_minute: Some(100),
            per_day: None,
            per_week: None,
        };
        let reservation = store
            .reserve_tokens("stream-drop-key", 100, &limits, Utc::now())
            .await
            .unwrap();
        let body = Body::from_stream(stream::pending::<
            Result<Bytes, axum_core::Error>,
        >());
        let response = wrap_streaming_settlement(
            HttpResponse::builder().body(body).unwrap(),
            store.clone(),
            reservation,
        );

        drop(response);
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;

        assert!(
            store
                .reserve_tokens("stream-drop-key", 100, &limits, Utc::now())
                .await
                .is_ok()
        );
    }
}
