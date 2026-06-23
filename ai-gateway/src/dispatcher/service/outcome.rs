use bytes::Bytes;
use chrono::{DateTime, Utc};
use http::{HeaderMap, StatusCode};
use tokio::{sync::oneshot, time::Instant};
use url::Url;
use uuid::Uuid;

use super::Dispatcher;
use crate::{
    config::credentials::ProviderCredentialId,
    dispatcher::extensions::ExtensionsCopier,
    error::api::ApiError,
    metrics::llm::TokenUsage,
    types::{
        body::{Body, BodyReader},
        extensions::{
            MapperContext, PromptContext, RequestContext, RequestKind,
            RouterRuntimeLabels, UpstreamAttemptContext,
        },
        provider::InferenceProvider,
        router::RouterId,
    },
};

/// Buffered provider result — same shape whether upstream is reqwest or an
/// in-process executor (chatgpt-web).
pub struct DispatchOutcome {
    pub response: http::Response<Body>,
    pub body_reader: BodyReader,
    pub tfft_rx: oneshot::Receiver<()>,
    pub target_url: Url,
    pub req_body_bytes: Bytes,
    pub request_headers: HeaderMap,
}

pub fn outcome_from_bytes(
    status: StatusCode,
    response_headers: HeaderMap,
    body: &Bytes,
    target_url: Url,
    req_body_bytes: Bytes,
    request_headers: HeaderMap,
) -> Result<DispatchOutcome, crate::error::internal::InternalError> {
    let stream =
        futures::stream::once(futures::future::ok::<_, ApiError>(body.clone()));
    let (resp_body, body_reader, tfft_rx) =
        BodyReader::wrap_stream(stream, false);
    let mut response = http::Response::builder()
        .status(status)
        .body(resp_body)
        .map_err(crate::error::internal::InternalError::HttpError)?;
    *response.headers_mut() = response_headers;
    Ok(DispatchOutcome {
        response,
        body_reader,
        tfft_rx,
        target_url,
        req_body_bytes,
        request_headers,
    })
}

async fn buffer_non_stream_response(
    response: http::Response<Body>,
    start_instant: Instant,
) -> (
    http::Response<Body>,
    BodyReader,
    oneshot::Receiver<()>,
    TokenUsage,
    Option<f64>,
) {
    let (parts, body) = response.into_parts();
    let (collected, tfft_ms) =
        collect_body_with_tfft(body, start_instant).await;
    let reported_usage = crate::metrics::llm::extract_usage_from_response_body(
        &collected, false,
    );
    let stream = futures::stream::once(futures::future::ready(Ok(collected)));
    let (new_body, body_reader, tfft_rx) =
        BodyReader::wrap_stream(stream, false);
    (
        http::Response::from_parts(parts, new_body),
        body_reader,
        tfft_rx,
        reported_usage,
        tfft_ms,
    )
}

async fn collect_body_with_tfft(
    body: Body,
    start_instant: Instant,
) -> (Bytes, Option<f64>) {
    use bytes::BytesMut;
    use futures::StreamExt;

    let mut buf = BytesMut::new();
    let mut tfft_ms = None;
    let mut stream = body.into_data_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.expect("response body is infallible");
        if tfft_ms.is_none() {
            tfft_ms = Some(start_instant.elapsed().as_secs_f64() * 1000.0);
        }
        buf.extend_from_slice(&chunk);
    }
    (buf.freeze(), tfft_ms)
}

pub struct FinalizeDispatchContext<'a> {
    pub mapper_ctx: MapperContext,
    pub req_ctx: &'a RequestContext,
    pub api_endpoint: Option<crate::endpoints::ApiEndpoint>,
    pub inference_provider: InferenceProvider,
    pub router_id: Option<RouterId>,
    pub start_instant: Instant,
    pub start_time: DateTime<Utc>,
    pub request_kind: RequestKind,
    pub prompt_ctx: Option<PromptContext>,
    pub router_runtime_labels: Option<RouterRuntimeLabels>,
    pub extracted_path_and_query: http::uri::PathAndQuery,
    pub upstream_attempt: Option<UpstreamAttemptContext>,
    pub credential_id: Option<ProviderCredentialId>,
}

impl Dispatcher {
    /// Shared observability tail for every provider backend (proxy or
    /// embedded).
    #[allow(clippy::too_many_lines)]
    pub(super) async fn finalize_dispatch(
        &self,
        mut outcome: DispatchOutcome,
        ctx: FinalizeDispatchContext<'_>,
    ) -> Result<http::Response<Body>, ApiError> {
        let FinalizeDispatchContext {
            mapper_ctx,
            req_ctx,
            api_endpoint,
            inference_provider,
            router_id,
            start_instant,
            start_time,
            request_kind,
            prompt_ctx,
            router_runtime_labels,
            extracted_path_and_query,
            upstream_attempt,
            credential_id,
        } = ctx;

        tracing::info!(
            target_url = %outcome.target_url,
            is_stream = %mapper_ctx.is_stream,
            response_status = %outcome.response.status(),
            provider = %self.provider,
            "provider dispatch completed"
        );

        let helicone_request_id = Uuid::new_v4();
        outcome.response.headers_mut().insert(
            "helicone-id",
            http::HeaderValue::from_str(&helicone_request_id.to_string())
                .expect("valid uuid"),
        );

        let auth_ctx = req_ctx.auth_context.as_ref();
        let extensions_copier = ExtensionsCopier::builder()
            .inference_provider(inference_provider)
            .router_id(router_id.clone())
            .auth_context(auth_ctx.cloned())
            .provider_request_id(None)
            .mapper_ctx(mapper_ctx.clone())
            .build();
        extensions_copier.copy_extensions(outcome.response.extensions_mut());
        outcome.response.extensions_mut().insert(mapper_ctx.clone());
        if let Some(api_endpoint) = api_endpoint.clone() {
            outcome.response.extensions_mut().insert(api_endpoint);
        }
        outcome
            .response
            .extensions_mut()
            .insert(extracted_path_and_query);

        let response_status = outcome.response.status();
        let response_headers = outcome.response.headers();
        let provider_metric_attrs = crate::metrics::llm::provider_attrs(
            &self.provider,
            mapper_ctx.model.as_ref(),
            router_id.as_ref(),
            outcome.target_url.path(),
            response_status,
            mapper_ctx.is_stream,
            request_kind,
        );
        self.app_state
            .0
            .metrics
            .llm
            .provider_requests
            .add(1, &provider_metric_attrs);
        self.app_state
            .0
            .metrics
            .llm
            .provider_request_body_bytes
            .add(
                u64::try_from(outcome.req_body_bytes.len()).unwrap_or(u64::MAX),
                &provider_metric_attrs,
            );
        self.app_state.0.metrics.llm.record_rate_limit_headers(
            response_headers,
            &provider_metric_attrs,
        );
        self.handle_error_and_rate_limiting(
            response_status,
            response_headers,
            api_endpoint,
            mapper_ctx.model.clone(),
        )
        .await?;

        let (body_reader, tfft_rx, reported_usage, tfft_ms) =
            if mapper_ctx.is_stream {
                (
                    outcome.body_reader,
                    outcome.tfft_rx,
                    TokenUsage::default(),
                    None,
                )
            } else {
                let (response, body_reader, tfft_rx, reported_usage, tfft_ms) =
                    buffer_non_stream_response(outcome.response, start_instant)
                        .await;
                outcome.response = response;
                (body_reader, tfft_rx, reported_usage, tfft_ms)
            };

        let usage_input = crate::metrics::provider::DispatchMetricsInput {
            app_state: &self.app_state,
            provider: &self.provider,
            credential: credential_id.as_ref(),
            model: mapper_ctx.model.as_ref(),
            router_id: router_id.as_ref(),
            attempt: upstream_attempt.as_ref(),
            status: response_status,
            stream: mapper_ctx.is_stream,
            request_kind,
            duration_ms: start_instant.elapsed().as_secs_f64() * 1000.0,
            tfft_ms,
            reported_usage,
            request_body: Some(&outcome.req_body_bytes),
            failover_class: None,
            agent_name: None,
        };
        crate::metrics::provider::attach_usage_header(
            &self.app_state,
            &mut outcome.response,
            &usage_input,
        );

        self.handle_logging(
            req_ctx,
            start_time,
            start_instant,
            outcome.target_url.clone(),
            outcome.request_headers.clone(),
            outcome.req_body_bytes.clone(),
            &outcome.response,
            body_reader,
            tfft_rx,
            &mapper_ctx,
            router_id,
            helicone_request_id,
            prompt_ctx,
            request_kind,
            router_runtime_labels,
            upstream_attempt.as_ref(),
            credential_id,
        );

        Ok(outcome.response)
    }
}

#[cfg(test)]
mod tests {
    use bytes::Bytes;
    use http::{HeaderMap, StatusCode};
    use http_body_util::BodyExt;
    use tokio::time::Instant;
    use url::Url;

    use super::{collect_body_with_tfft, outcome_from_bytes};
    use crate::{error::api::ApiError, types::body::Body};

    #[tokio::test]
    async fn collect_body_with_tfft_records_first_byte_latency() {
        use std::time::Duration;

        use futures::stream;

        let (tx, rx) = tokio::sync::oneshot::channel();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(20)).await;
            let _ = tx.send(Bytes::from_static(b"hello"));
        });
        let body = Body::from_stream(stream::once(async {
            rx.await.map_err(|_| {
                ApiError::Internal(
                    crate::error::internal::InternalError::Internal,
                )
            })
        }));
        let start = Instant::now();
        let (bytes, tfft_ms) = collect_body_with_tfft(body, start).await;
        assert_eq!(bytes, Bytes::from_static(b"hello"));
        let tfft = tfft_ms.expect("ttft should be measured");
        assert!(tfft >= 15.0, "expected at least 15ms, got {tfft}");
        assert!(tfft <= start.elapsed().as_secs_f64() * 1000.0);
    }

    #[tokio::test]
    async fn outcome_from_bytes_wraps_body_for_logging_pipeline() {
        let target =
            Url::parse("https://chatgpt.com/backend-api/f/conversation")
                .unwrap();
        let body = Bytes::from_static(br#"{"ok":true}"#);
        let req = Bytes::from_static(br#"{"model":"gpt-5.5-instant"}"#);

        let outcome = outcome_from_bytes(
            StatusCode::OK,
            HeaderMap::new(),
            &body,
            target.clone(),
            req.clone(),
            HeaderMap::new(),
        )
        .unwrap();

        assert_eq!(outcome.response.status(), StatusCode::OK);
        assert_eq!(outcome.target_url, target);
        assert_eq!(outcome.req_body_bytes, req);

        let collected = outcome
            .response
            .into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes();
        assert_eq!(collected, body);
    }
}

#[cfg(all(test, feature = "testing"))]
mod finalize_tests {
    use std::str::FromStr;

    use bytes::Bytes;
    use chrono::Utc;
    use compact_str::CompactString;
    use http::{HeaderMap, StatusCode};
    use url::Url;

    use super::{FinalizeDispatchContext, outcome_from_bytes};
    use crate::{
        app_state::AppState,
        dispatcher::{client::Client, service::Dispatcher},
        types::{
            extensions::{MapperContext, RequestContext, RequestKind},
            model_id::ModelId,
            provider::InferenceProvider,
            router::RouterId,
        },
    };

    async fn test_dispatcher(app_state: AppState) -> Dispatcher {
        let provider = InferenceProvider::Named("chatgpt-web".into());
        let client = Client::new(&app_state, provider.clone())
            .await
            .expect("client");
        Dispatcher {
            client,
            app_state,
            provider,
            rate_limit_tx: None,
        }
    }

    #[tokio::test]
    async fn finalize_dispatch_attaches_observability_extensions() {
        let app_state = AppState::test_default().await;
        let dispatcher = test_dispatcher(app_state).await;
        let target =
            Url::parse("https://chatgpt.com/backend-api/f/conversation")
                .unwrap();
        let body = Bytes::from_static(b"{\"choices\":[]}");
        let outcome = outcome_from_bytes(
            StatusCode::OK,
            HeaderMap::new(),
            &body,
            target,
            Bytes::from_static(b"{}"),
            HeaderMap::new(),
        )
        .unwrap();

        let mapper_ctx = MapperContext {
            is_stream: false,
            model: Some(
                ModelId::from_str("chatgpt-web/gpt-5.5-instant").unwrap(),
            ),
        };
        let req_ctx = RequestContext {
            router_config: None,
            auth_context: None,
        };
        let path = http::uri::PathAndQuery::from_static("/v1/chat/completions");

        let response = dispatcher
            .finalize_dispatch(
                outcome,
                FinalizeDispatchContext {
                    mapper_ctx: mapper_ctx.clone(),
                    req_ctx: &req_ctx,
                    api_endpoint: None,
                    inference_provider: InferenceProvider::Named(
                        "chatgpt-web".into(),
                    ),
                    router_id: Some(RouterId::Named(CompactString::new(
                        "autodefault",
                    ))),
                    start_instant: tokio::time::Instant::now(),
                    start_time: Utc::now(),
                    request_kind: RequestKind::Router,
                    prompt_ctx: None,
                    router_runtime_labels: None,
                    extracted_path_and_query: path.clone(),
                    upstream_attempt: None,
                    credential_id: None,
                },
            )
            .await
            .expect("finalize");

        assert!(response.headers().get("helicone-id").is_some());
        assert!(response.extensions().get::<MapperContext>().is_some());
        assert_eq!(
            response.extensions().get::<http::uri::PathAndQuery>(),
            Some(&path)
        );
    }
}
