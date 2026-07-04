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
    config::credentials::ProviderCredentialId,
    logger::service::LoggerService,
    metrics::{
        provider::{
            DispatchMetricsInput, RecordAttemptInput, build_attempt_record,
            emit_pending_route_trace, generation_ms_per_output_token,
            record_upstream_attempt,
        },
        tfft::TFFTFuture,
    },
    types::{
        body::{Body, BodyReader},
        extensions::{
            MapperContext, PendingRouteTrace, PromptContext, RequestContext,
            RequestKind, RouterRuntimeLabels, UpstreamAttemptContext,
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
        router_runtime_labels: Option<RouterRuntimeLabels>,
        upstream_attempt: Option<&UpstreamAttemptContext>,
        credential_id: Option<ProviderCredentialId>,
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
                .router_runtime_labels(router_runtime_labels.clone())
                .upstream_attempt(upstream_attempt.cloned())
                .credential_id(credential_id.clone())
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
                router_id,
                request_kind,
                router_runtime_labels,
                req_body_len: req_body_bytes.len(),
                req_body_bytes,
                provider: self.provider.clone(),
                upstream_attempt: upstream_attempt.cloned(),
                credential_id,
                pending_route_trace: client_response
                    .extensions()
                    .get::<PendingRouteTrace>()
                    .cloned(),
            });
        }
    }

    #[allow(clippy::too_many_lines)]
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
            router_runtime_labels,
            req_body_len,
            req_body_bytes,
            provider,
            upstream_attempt,
            credential_id,
            pending_route_trace,
        } = context;
        let mapper_ctx = mapper_ctx.clone();
        let app_state = self.app_state.clone();
        let provider_metric_attrs = crate::metrics::llm::provider_attrs(
            &self.provider,
            mapper_ctx.model.as_ref(),
            router_id.as_ref(),
            target_url.path(),
            response_status,
            mapper_ctx.is_stream,
            request_kind,
        );
        let tfft_attrs = provider_metric_attrs.clone();
        let is_stream = mapper_ctx.is_stream;
        let path = target_url.path().to_string();
        let agent_name = pending_route_trace
            .as_ref()
            .and_then(|trace| trace.agent_name.clone());
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
                let duration_ms = start_instant.elapsed().as_secs_f64() * 1000.0;
                app_state.0.metrics.llm.provider_response_duration.record(
                    duration_ms,
                    &provider_metric_attrs,
                );
                let reported_usage =
                    crate::metrics::llm::extract_usage_from_response_body(
                        &response_body,
                        is_stream,
                    );
                if !reported_usage.is_empty() {
                    app_state
                        .0
                        .metrics
                        .llm
                        .record_provider_tokens(reported_usage, &provider_metric_attrs);
                }
                let tfft_ok_ms = tfft_duration
                    .as_ref()
                    .ok()
                    .map(|d| d.as_secs_f64() * 1000.0);
                if let Ok(dur) = &tfft_duration {
                    let mut attrs = tfft_attrs;
                    attrs.push(KeyValue::new("path", path));
                    app_state
                        .0
                        .metrics
                        .tfft_duration
                        .record(dur.as_secs_f64() * 1000.0, &attrs);
                }

                record_upstream_attempt(&DispatchMetricsInput {
                    app_state: &app_state,
                    provider: &provider,
                    credential: credential_id.as_ref(),
                    model: mapper_ctx.model.as_ref(),
                    router_id: router_id.as_ref(),
                    attempt: upstream_attempt.as_ref(),
                    status: response_status,
                    stream: is_stream,
                    request_kind,
                    duration_ms,
                    tfft_ms: tfft_ok_ms,
                    reported_usage,
                    request_body: Some(&req_body_bytes),
                    failover_class: None,
                    agent_name: agent_name.as_deref(),
                });

                if matches!(
                    request_kind,
                    RequestKind::DirectProxy | RequestKind::Managed
                ) {
                    app_state.0.metrics.provider.record_client_request(false);
                }

                if let Some(ref rtl) = router_runtime_labels {
                    app_state.runtime_metrics().record_router_complete(
                        rtl,
                        &provider,
                        mapper_ctx.model.as_ref(),
                        response_status,
                        req_body_len,
                        response_body.len(),
                        duration_ms,
                        tfft_ok_ms,
                        reported_usage,
                        false,
                    );
                }

                if let Some(pending) = pending_route_trace {
                    let record = build_attempt_record(&RecordAttemptInput {
                        provider: &provider,
                        credential: credential_id
                            .as_ref()
                            .map(ProviderCredentialId::as_str)
                            .or_else(|| {
                                upstream_attempt
                                    .as_ref()
                                    .map(|a| a.credential.as_str())
                            })
                            .unwrap_or("default"),
                        model: mapper_ctx.model.as_ref(),
                        router_id: router_id.as_ref(),
                        attempt: upstream_attempt.as_ref(),
                        status: response_status,
                        stream: is_stream,
                        request_kind,
                        duration_ms,
                        tfft_ms: tfft_ok_ms,
                        reported_usage,
                        request_body: Some(&req_body_bytes),
                        estimate_tokens: app_state
                            .config()
                            .observability
                            .estimate_tokens,
                        failover_class: None,
                        agent_name: agent_name.as_deref(),
                    });
                    let usage_source = match record.usage_source {
                        crate::metrics::provider::attempt::UsageSource::Reported => {
                            "reported"
                        }
                        crate::metrics::provider::attempt::UsageSource::Estimated => {
                            "estimated"
                        }
                        crate::metrics::provider::attempt::UsageSource::None => "none",
                    };
                    emit_pending_route_trace(
                        &pending,
                        generation_ms_per_output_token(&record),
                        Some(usage_source),
                    );
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
    router_id: Option<RouterId>,
    request_kind: RequestKind,
    router_runtime_labels: Option<RouterRuntimeLabels>,
    req_body_len: usize,
    req_body_bytes: Bytes,
    provider: crate::types::provider::InferenceProvider,
    upstream_attempt: Option<UpstreamAttemptContext>,
    credential_id: Option<ProviderCredentialId>,
    pending_route_trace: Option<PendingRouteTrace>,
}
