use opentelemetry::global;
use tower::{ServiceBuilder, buffer::BufferLayer};
use tower_http::{
    ServiceBuilderExt,
    catch_panic::CatchPanicLayer,
    compression::CompressionLayer,
    cors::{Any, CorsLayer},
    normalize_path::NormalizePathLayer,
    sensitive_headers::SetSensitiveHeadersLayer,
    trace::TraceLayer,
};
use tracing::Level;
use telemetry::{make_span::SpanFactory, tracing::MakeRequestId};
use crate::{
    app_state::AppState,
    error::init::InitError,
    metrics::{self, attribute_extractor::AttributeExtractor},
    middleware::response_headers::ResponseHeaderLayer,
    router::meta::MetaRouter,
    utils::{
        catch_panic::PanicResponder, handle_error::ErrorHandlerLayer,
        health_check::HealthCheckLayer, timer::TimerLayer,
        validate_config::ValidateRouterConfigLayer,
    },
};
use super::BoxedServiceStack;

pub async fn build_service_stack(app_state: AppState) -> Result<BoxedServiceStack, InitError> {
    let meter = global::meter("ai-gateway");
    let otel_metrics_layer = opentelemetry_instrumentation_tower::HTTPMetricsLayerBuilder::builder()
        .with_meter(meter)
        .with_response_extractor::<_, axum_core::body::Body>(AttributeExtractor)
        .build()?;

    let router = MetaRouter::build(app_state.clone()).await?;

    let compression_layer = CompressionLayer::new().gzip(true).br(true).deflate(true).zstd(true);
    let cors_layer = CorsLayer::new().allow_headers(Any).allow_methods(Any).allow_origin(Any);

    let service_stack = ServiceBuilder::new()
        .layer(CatchPanicLayer::custom(PanicResponder))
        .layer(SetSensitiveHeadersLayer::new(std::iter::once(http::header::AUTHORIZATION)))
        .layer(TraceLayer::new_for_http()
            .make_span_with(SpanFactory::new(Level::INFO, app_state.config().telemetry.propagate))
            .on_body_chunk(())
            .on_eos(()))
        .layer(otel_metrics_layer)
        .set_x_request_id(MakeRequestId)
        .propagate_x_request_id()
        .layer(NormalizePathLayer::trim_trailing_slash())
        .layer(metrics::request_count::Layer::new(app_state.clone()))
        .layer(compression_layer)
        .layer(cors_layer)
        .layer(HealthCheckLayer::new())
        .layer(ValidateRouterConfigLayer::new())
        .layer(TimerLayer::new())
        .layer(ErrorHandlerLayer::new(app_state.clone()))
        .layer(ResponseHeaderLayer::new(app_state.response_headers_config()))
        .map_err(crate::error::internal::InternalError::BufferError)
        .layer(BufferLayer::new(1024))
        .layer(ErrorHandlerLayer::new(app_state.clone()))
        .service(router);

    Ok(tower::util::BoxCloneService::new(service_stack))
}
