use bytes::Bytes;
use chrono::{DateTime, Utc};
use http::HeaderMap;
use http_body_util::BodyExt;
use opentelemetry::KeyValue;
use tokio::{sync::oneshot, time::Instant};
use tracing::Instrument;
use uuid::Uuid;

use super::Dispatcher;
use crate::{
    logger::service::LoggerService,
    metrics::tfft::TFFTFuture,
    types::{
        body::{Body, BodyReader},
        extensions::{
            MapperContext, PromptContext, RequestContext, RequestKind,
        },
        router::RouterId,
    },
};

impl Dispatcher {
    #[allow(clippy::too_many_arguments)]
    pub fn handle_logging(
        &self,
        req_ctx: &RequestContext,
        start_time: DateTime<Utc>,
        start_instant: Instant,
        target_url: url::Url,
        headers: HeaderMap,
        req_body_bytes: Bytes,
        client_response: &http::Response<Body>,
        response_body_for_logger: BodyReader,
        tfft_rx: oneshot::Receiver<()>,
        mapper_ctx: &MapperContext,
        router_id: Option<RouterId>,
        helicone_request_id: Uuid,
        prompt_ctx: Option<PromptContext>,
        request_kind: RequestKind,
    ) {
        let deployment_target =
            self.app_state.config().deployment_target.clone();
        if self.app_state.config().helicone.is_observability_enabled()
            && let Some(auth_ctx) = req_ctx.auth_context.clone()
        {
            let response_logger = LoggerService::builder()
                .app_state(self.app_state.clone())
                .auth_ctx(auth_ctx)
                .start_time(start_time)
                .start_instant(start_instant)
                .target_url(target_url)
                .request_headers(headers)
                .request_body(req_body_bytes)
                .response_status(client_response.status())
                .response_body(response_body_for_logger)
                .provider(self.provider.clone())
                .tfft_rx(tfft_rx)
                .mapper_ctx(mapper_ctx.clone())
                .router_id(router_id)
                .deployment_target(deployment_target)
                .request_id(helicone_request_id)
                .prompt_ctx(prompt_ctx)
                .request_kind(request_kind)
                .build();
            let app_state = self.app_state.clone();
            tokio::spawn(
                async move {
                    if let Err(e) = response_logger.log().await {
                        app_state.0.metrics.error_count.add(
                            1,
                            &[KeyValue::new("type", e.as_ref().to_string())],
                        );
                    }
                }
                .instrument(tracing::Span::current()),
            );
        } else {
            self.handle_metrics_logging(MetricsLoggingContext {
                start_instant,
                response_body: response_body_for_logger,
                response_status: client_response.status(),
                tfft_rx,
                mapper_ctx,
                target_url: &target_url,
                router_id: router_id.as_ref(),
                request_kind,
            });
        }
    }

    fn handle_metrics_logging(&self, context: MetricsLoggingContext<'_>) {
        let MetricsLoggingContext {
            start_instant,
            response_body,
            response_status,
            tfft_rx,
            mapper_ctx,
            target_url,
            router_id,
            request_kind,
        } = context;
        let app_state = self.app_state.clone();
        let provider_metric_attrs = crate::metrics::llm::provider_attrs(
            &self.provider,
            mapper_ctx.model.as_ref(),
            router_id,
            target_url.path(),
            response_status,
            mapper_ctx.is_stream,
            request_kind,
        );
        let tfft_attrs = provider_metric_attrs.clone();
        let is_stream = mapper_ctx.is_stream;
        let path = target_url.path().to_string();
        tokio::spawn(
            async move {
                let tfft_future = TFFTFuture::new(start_instant, tfft_rx);
                let collect_future = response_body.collect();
                let (response_body, tfft_duration) =
                    tokio::join!(collect_future, tfft_future);
                let response_body = response_body
                    .inspect_err(|_| tracing::error!("infallible errored"))
                    .expect("infallible never errors")
                    .to_bytes();
                app_state.0.metrics.llm.provider_response_body_bytes.add(
                    u64::try_from(response_body.len()).unwrap_or(u64::MAX),
                    &provider_metric_attrs,
                );
                app_state.0.metrics.llm.provider_response_duration.record(
                    start_instant.elapsed().as_secs_f64() * 1000.0,
                    &provider_metric_attrs,
                );
                let usage =
                    crate::metrics::llm::extract_usage_from_response_body(
                        &response_body,
                        is_stream,
                    );
                if !usage.is_empty() {
                    app_state
                        .0
                        .metrics
                        .llm
                        .record_provider_tokens(usage, &provider_metric_attrs);
                }
                if let Ok(dur) = tfft_duration {
                    let mut attrs = tfft_attrs;
                    attrs.push(KeyValue::new("path", path));
                    app_state
                        .0
                        .metrics
                        .tfft_duration
                        .record(dur.as_secs_f64() * 1000.0, &attrs);
                }
            }
            .instrument(tracing::Span::current()),
        );
    }
}

struct MetricsLoggingContext<'a> {
    start_instant: Instant,
    response_body: BodyReader,
    response_status: http::StatusCode,
    tfft_rx: oneshot::Receiver<()>,
    mapper_ctx: &'a MapperContext,
    target_url: &'a url::Url,
    router_id: Option<&'a RouterId>,
    request_kind: RequestKind,
}
