use super::Dispatcher;
use crate::{
    logger::service::LoggerService,
    metrics::tfft::TFFTFuture,
    types::{
        body::{Body, BodyReader},
        extensions::{MapperContext, PromptContext, RequestContext},
        router::RouterId,
    },
};
use bytes::Bytes;
use chrono::{DateTime, Utc};
use http::HeaderMap;
use opentelemetry::KeyValue;
use tokio::{sync::oneshot, time::Instant};
use tracing::Instrument;
use uuid::Uuid;

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
    ) {
        let deployment_target =
            self.app_state.config().deployment_target.clone();
        if self.app_state.config().helicone.is_observability_enabled() {
            if let Some(auth_ctx) = req_ctx.auth_context.clone() {
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
                    .build();
                let app_state = self.app_state.clone();
                tokio::spawn(
                    async move {
                        if let Err(e) = response_logger.log().await {
                            app_state.0.metrics.error_count.add(
                                1,
                                &[KeyValue::new(
                                    "type",
                                    e.as_ref().to_string(),
                                )],
                            );
                        }
                    }
                    .instrument(tracing::Span::current()),
                );
            }
        } else {
            self.handle_metrics_logging(
                start_instant,
                tfft_rx,
                mapper_ctx,
                target_url,
            );
        }
    }

    fn handle_metrics_logging(
        &self,
        start_instant: Instant,
        tfft_rx: oneshot::Receiver<()>,
        mapper_ctx: &MapperContext,
        target_url: url::Url,
    ) {
        let app_state = self.app_state.clone();
        let model = mapper_ctx
            .model
            .as_ref()
            .map_or_else(|| "unknown".to_string(), |m| m.to_string());
        let path = target_url.path().to_string();
        let provider = self.provider.to_string();
        tokio::spawn(
            async move {
                let tfft_future = TFFTFuture::new(start_instant, tfft_rx);
                let (_resp, tfft_duration) =
                    tokio::join!(tokio::spawn(async move {}), tfft_future); // placeholder
                if let Ok(dur) = tfft_duration {
                    let attrs = [
                        KeyValue::new("provider", provider),
                        KeyValue::new("model", model),
                        KeyValue::new("path", path),
                    ];
                    app_state
                        .0
                        .metrics
                        .tfft_duration
                        .record(dur.as_millis() as f64, &attrs);
                }
            }
            .instrument(tracing::Span::current()),
        );
    }
}
