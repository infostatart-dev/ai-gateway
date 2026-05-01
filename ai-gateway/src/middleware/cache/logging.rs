use std::str::FromStr;

use chrono::{DateTime, Utc};
use http::request::Parts;
use http_body_util::BodyExt;
use opentelemetry::KeyValue;
use tokio::{sync::oneshot, time::Instant};
use tracing::Instrument;
use uuid::Uuid;

use super::context::CacheContext;
use crate::{
    app_state::AppState,
    logger::service::LoggerService,
    metrics::tfft::TFFTFuture,
    types::{
        body::{Body, BodyReader},
        extensions::{AuthContext, MapperContext},
        model_id::ModelId,
        provider::InferenceProvider,
    },
};

pub const DEFAULT_UUID: Uuid = Uuid::from_u128(0);

#[allow(clippy::too_many_arguments)]
pub fn spawn_cache_logging(
    app_state: AppState,
    req_parts: Parts,
    req_body: Body,
    body_reader: BodyReader,
    tfft_rx: oneshot::Receiver<()>,
    start_time: DateTime<Utc>,
    start_instant: Instant,
    target_url: url::Url,
    req_headers: http::HeaderMap,
    status: http::StatusCode,
    ctx: &CacheContext,
    resp_headers: &http::HeaderMap,
) {
    if app_state.config().helicone.is_observability_enabled() {
        let auth_ctx = req_parts.extensions.get::<AuthContext>().cloned();
        let app_state_cloned = app_state.clone();
        let buckets = ctx.buckets;
        let directive = ctx.directive.clone();
        let helicone_request_id = resp_headers
            .get("helicone-id")
            .and_then(|hv| Uuid::parse_str(hv.to_str().unwrap()).ok())
            .unwrap_or(DEFAULT_UUID);
        let router_id = req_parts
            .extensions
            .get::<crate::types::router::RouterId>()
            .cloned();
        let deployment_target = app_state.config().deployment_target.clone();

        tokio::spawn(
            async move {
                let req_body_bytes = req_body
                    .collect()
                    .await
                    .ok()
                    .map(|b| b.to_bytes())
                    .unwrap_or_default();
                let Ok(deserialized_body) = serde_json::from_slice::<
                    async_openai::types::chat::CreateChatCompletionRequest,
                >(&req_body_bytes) else {
                    return;
                };
                let Ok(model) = ModelId::from_str(&deserialized_body.model)
                else {
                    return;
                };
                let provider = model
                    .inference_provider()
                    .unwrap_or(InferenceProvider::OpenAI);
                let mapper_ctx = MapperContext {
                    is_stream: deserialized_body.stream.unwrap_or(false),
                    model: Some(model),
                };

                let response_logger = LoggerService::builder()
                    .app_state(app_state.clone())
                    .auth_ctx(auth_ctx.unwrap())
                    .start_time(start_time)
                    .start_instant(start_instant)
                    .target_url(target_url)
                    .request_headers(req_headers)
                    .request_body(req_body_bytes)
                    .response_status(status)
                    .response_body(body_reader)
                    .provider(provider)
                    .tfft_rx(tfft_rx)
                    .mapper_ctx(mapper_ctx)
                    .router_id(router_id)
                    .deployment_target(deployment_target)
                    .cache_enabled(Some(true))
                    .cache_bucket_max_size(buckets)
                    .cache_control(directive)
                    .cache_reference_id(Some(helicone_request_id.to_string()))
                    .request_id(helicone_request_id)
                    .build();
                if let Err(e) = response_logger.log().await {
                    app_state_cloned.0.metrics.error_count.add(
                        1,
                        &[KeyValue::new("type", e.as_ref().to_string())],
                    );
                }
            }
            .instrument(tracing::Span::current()),
        );
    } else {
        tokio::spawn(
            async move {
                let tfft_future = TFFTFuture::new(start_instant, tfft_rx);
                let (_response_body, tfft_duration) =
                    tokio::join!(body_reader.collect(), tfft_future);
                if let Ok(tfft_duration) = tfft_duration {
                    app_state.0.metrics.tfft_duration.record(
                        tfft_duration.as_millis() as f64,
                        &[KeyValue::new("path", target_url.path().to_string())],
                    );
                }
            }
            .instrument(tracing::Span::current()),
        );
    }
}
