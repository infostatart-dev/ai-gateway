pub mod make_span;
pub mod tracing;
pub mod utils;

use std::sync::OnceLock;

use opentelemetry::{
    TraceId, global,
    trace::{TracerProvider, noop::NoopTextMapPropagator},
};
use opentelemetry_otlp::{
    ExporterBuildError, LogExporter, MetricExporter, SpanExporter,
    WithExportConfig,
};
use opentelemetry_sdk::{
    Resource,
    logs::SdkLoggerProvider,
    metrics::SdkMeterProvider,
    propagation::TraceContextPropagator,
    trace::{IdGenerator, SdkTracerProvider},
};
use prometheus::Encoder;
use serde::{Deserialize, Serialize};
pub use tracing_subscriber::util::TryInitError;
use tracing_subscriber::{
    EnvFilter, Layer, filter::ParseError, layer::SubscriberExt,
    util::SubscriberInitExt,
};
use utils::default_true;
use uuid::Uuid;

static PROMETHEUS_REGISTRY: OnceLock<prometheus::Registry> = OnceLock::new();
const MAX_EVENTS_PER_SPAN: u32 = 256;
const MAX_ATTRIBUTES_PER_SPAN: u32 = 128;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, Hash)]
#[serde(default, deny_unknown_fields, rename_all = "kebab-case")]
pub struct Config {
    /// Logging and tracing level in the env logger format.
    #[serde(default = "default_level")]
    pub level: String,
    #[serde(default = "default_service_name")]
    pub service_name: String,
    #[serde(default)]
    pub exporter: Exporter,
    #[serde(default = "default_otlp_endpoint")]
    pub otlp_endpoint: String,
    #[serde(default = "default_true")]
    pub otlp_logs: bool,
    #[serde(default = "default_true")]
    pub propagate: bool,
    #[serde(default)]
    pub format: Format,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            level: default_level(),
            service_name: default_service_name(),
            exporter: Exporter::default(),
            otlp_endpoint: default_otlp_endpoint(),
            otlp_logs: default_true(),
            propagate: default_true(),
            format: Format::default(),
        }
    }
}

#[derive(
    Default, Clone, Debug, Deserialize, Serialize, PartialEq, Eq, Hash,
)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub enum Exporter {
    #[default]
    Stdout,
    Otlp,
    Both,
}

#[derive(
    Default, Clone, Debug, Deserialize, Serialize, PartialEq, Eq, Hash,
)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub enum Format {
    #[default]
    Pretty,
    Compact,
    Json,
}

fn default_service_name() -> String {
    "ai-gateway".to_string()
}

fn default_level() -> String {
    "info".to_string()
}

fn default_otlp_endpoint() -> String {
    "http://localhost:4317/v1/metrics".to_string()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrometheusMetricsText {
    pub content_type: String,
    pub body: Vec<u8>,
}

#[derive(Debug, thiserror::Error)]
pub enum TelemetryError {
    #[error("Log exporter build error: {0}")]
    LogExporterBuild(ExporterBuildError),
    #[error("Trace exporter build error: {0}")]
    TraceExporterBuild(ExporterBuildError),
    #[error("Metric exporter build error: {0}")]
    MetricExporterBuild(ExporterBuildError),
    #[error("Prometheus exporter build error: {0}")]
    PrometheusExporterBuild(opentelemetry_sdk::error::OTelSdkError),
    #[error("Invalid log directive: {0}")]
    InvalidLogDirective(#[from] ParseError),
    #[error("Subscriber error: {0}")]
    Subscriber(#[from] TryInitError),
    #[error("Otel http metrics error")]
    OtelHttpMetrics,
}

fn resource(config: &Config) -> Resource {
    Resource::builder()
        .with_service_name(config.service_name.clone())
        .build()
}

/// Initialize telemetry with the given config.
///
/// # Notes
/// - The reason the `TracerProvider` is not optional is because without it we
///   don't generate trace ids, which is useful to have when
///   debugging/developing.
///
/// # Errors
/// If any of the configuration is invalid.
pub fn init_telemetry(
    config: &Config,
) -> Result<
    (
        Option<SdkLoggerProvider>,
        SdkTracerProvider,
        Option<SdkMeterProvider>,
    ),
    TelemetryError,
> {
    let resource = resource(config);

    if config.propagate {
        global::set_text_map_propagator(TraceContextPropagator::new());
    } else {
        global::set_text_map_propagator(NoopTextMapPropagator::new());
    }

    match config.exporter {
        Exporter::Stdout => {
            let tracer_provider = init_stdout(&resource, config)?;
            let metrics_provider = prometheus_metrics_provider(resource)?;
            global::set_meter_provider(metrics_provider.clone());
            Ok((None, tracer_provider, Some(metrics_provider)))
        }
        Exporter::Otlp => {
            let (logger_provider, tracer_provider, metrics_provider) =
                init_otlp(config)?;
            Ok((logger_provider, tracer_provider, Some(metrics_provider)))
        }
        Exporter::Both => {
            let (logger_provider, tracer_provider, metrics_provider) =
                init_otlp_with_stdout(config)?;
            Ok((logger_provider, tracer_provider, Some(metrics_provider)))
        }
    }
}

fn init_otlp(
    config: &Config,
) -> Result<
    (
        Option<SdkLoggerProvider>,
        SdkTracerProvider,
        SdkMeterProvider,
    ),
    TelemetryError,
> {
    init_otlp_pipeline(config, false)
}

fn init_otlp_with_stdout(
    config: &Config,
) -> Result<
    (
        Option<SdkLoggerProvider>,
        SdkTracerProvider,
        SdkMeterProvider,
    ),
    TelemetryError,
> {
    init_otlp_pipeline(config, true)
}

fn init_otlp_pipeline(
    config: &Config,
    with_stdout: bool,
) -> Result<
    (
        Option<SdkLoggerProvider>,
        SdkTracerProvider,
        SdkMeterProvider,
    ),
    TelemetryError,
> {
    let resource = resource(config);

    // logging
    let (logger_provider, otel_layer) = if config.otlp_logs {
        let logger_provider = logger_provider(config, resource.clone())
            .map_err(TelemetryError::LogExporterBuild)?;
        let otel_layer =
            opentelemetry_appender_tracing::layer::OpenTelemetryTracingBridge::new(
                &logger_provider,
            )
            .with_filter(env_filter(config)?);
        (Some(logger_provider), Some(otel_layer))
    } else {
        (None, None)
    };

    // tracing
    let tracer_provider = tracer_provider(config, resource.clone())
        .map_err(TelemetryError::TraceExporterBuild)?;
    let tracer = tracer_provider.tracer(config.service_name.clone());
    let tracing_layer = tracing_opentelemetry::layer()
        .with_tracer(tracer)
        .with_filter(env_filter(config)?);

    let stdout_layer = if with_stdout {
        let layer = match config.format {
            Format::Pretty => tracing_subscriber::fmt::layer()
                .pretty()
                .with_file(true)
                .with_line_number(true)
                .with_filter(env_filter(config)?)
                .boxed(),
            Format::Compact => tracing_subscriber::fmt::layer()
                .compact()
                .with_file(true)
                .with_line_number(true)
                .with_filter(env_filter(config)?)
                .boxed(),
            Format::Json => tracing_subscriber::fmt::layer()
                .json()
                .with_file(true)
                .with_line_number(true)
                .with_filter(env_filter(config)?)
                .boxed(),
        };
        Some(layer)
    } else {
        None
    };

    tracing_subscriber::registry()
        .with(tracing_layer)
        .with(otel_layer)
        .with(stdout_layer)
        .try_init()?;

    // metrics
    let metrics_provider = metrics_provider(config, resource)?;

    global::set_meter_provider(metrics_provider.clone());
    global::set_tracer_provider(tracer_provider.clone());

    log_panics::init();

    Ok((logger_provider, tracer_provider, metrics_provider))
}

fn init_stdout(
    resource: &Resource,
    config: &Config,
) -> Result<SdkTracerProvider, TelemetryError> {
    // logging
    let fmt_layer = match config.format {
        Format::Pretty => tracing_subscriber::fmt::layer()
            .pretty()
            .with_file(true)
            .with_line_number(true)
            .with_filter(env_filter(config)?)
            .boxed(),
        Format::Compact => tracing_subscriber::fmt::layer()
            .compact()
            .with_file(true)
            .with_line_number(true)
            .with_filter(env_filter(config)?)
            .boxed(),
        Format::Json => tracing_subscriber::fmt::layer()
            .json()
            .with_file(true)
            .with_line_number(true)
            .with_filter(env_filter(config)?)
            .boxed(),
    };
    let registry = tracing_subscriber::registry().with(fmt_layer);

    // tracing
    let tracer_provider = tracer_provider(config, resource.clone())
        .map_err(TelemetryError::TraceExporterBuild)?;
    let tracer = tracer_provider.tracer(config.service_name.clone());
    let filter = env_filter(config)?;
    let tracing_layer = tracing_opentelemetry::layer()
        .with_tracer(tracer)
        .with_filter(filter);
    registry.with(tracing_layer).try_init()?;
    opentelemetry::global::set_tracer_provider(tracer_provider.clone());

    log_panics::init();

    Ok(tracer_provider)
}

fn env_filter(config: &Config) -> Result<EnvFilter, TelemetryError> {
    // we purposely avoid the EnvFilter::new API so we can catch invalid
    // directives
    let filter = EnvFilter::new(config.level.clone())
        // https://github.com/open-telemetry/opentelemetry-rust/issues/2877
        .add_directive("hyper=off".parse()?)
        .add_directive("tonic=off".parse()?)
        .add_directive("h2=off".parse()?)
        .add_directive("opentelemetry_sdk=error".parse()?)
        .add_directive("reqwest=off".parse()?);
    Ok(filter)
}

fn tracer_provider(
    config: &Config,
    resource: Resource,
) -> Result<SdkTracerProvider, ExporterBuildError> {
    match &config.exporter {
        Exporter::Stdout => {
            Ok(SdkTracerProvider::builder()
                .with_resource(resource)
                // we don't need an exporter here for stdout since we really
                // just want the tracer to generate trace ids
                .with_id_generator(UuidGenerator)
                .with_max_events_per_span(MAX_EVENTS_PER_SPAN)
                .with_max_attributes_per_span(MAX_ATTRIBUTES_PER_SPAN)
                .build())
        }
        Exporter::Otlp | Exporter::Both => {
            let exporter = SpanExporter::builder()
                .with_tonic()
                .with_endpoint(config.otlp_endpoint.clone())
                .build()?;
            let provider = SdkTracerProvider::builder()
                .with_resource(resource)
                .with_batch_exporter(exporter)
                .with_id_generator(UuidGenerator)
                .with_max_events_per_span(MAX_EVENTS_PER_SPAN)
                .with_max_attributes_per_span(MAX_ATTRIBUTES_PER_SPAN)
                .build();
            Ok(provider)
        }
    }
}

fn logger_provider(
    config: &Config,
    resource: Resource,
) -> Result<SdkLoggerProvider, ExporterBuildError> {
    let exporter = LogExporter::builder()
        .with_tonic()
        .with_endpoint(config.otlp_endpoint.clone())
        .build()?;
    Ok(SdkLoggerProvider::builder()
        .with_resource(resource)
        .with_batch_exporter(exporter)
        .build())
}

fn metrics_provider(
    config: &Config,
    resource: Resource,
) -> Result<SdkMeterProvider, TelemetryError> {
    let exporter = MetricExporter::builder()
        .with_tonic()
        .with_endpoint(config.otlp_endpoint.clone())
        .build()
        .map_err(TelemetryError::MetricExporterBuild)?;
    let prometheus_exporter = prometheus_exporter()?;
    Ok(SdkMeterProvider::builder()
        .with_reader(prometheus_exporter)
        .with_periodic_exporter(exporter)
        .with_resource(resource)
        .build())
}

fn prometheus_metrics_provider(
    resource: Resource,
) -> Result<SdkMeterProvider, TelemetryError> {
    let prometheus_exporter = prometheus_exporter()?;
    Ok(SdkMeterProvider::builder()
        .with_reader(prometheus_exporter)
        .with_resource(resource)
        .build())
}

fn prometheus_exporter()
-> Result<opentelemetry_prometheus::PrometheusExporter, TelemetryError> {
    opentelemetry_prometheus::exporter()
        .with_registry(prometheus_registry())
        .build()
        .map_err(TelemetryError::PrometheusExporterBuild)
}

fn prometheus_registry() -> prometheus::Registry {
    PROMETHEUS_REGISTRY
        .get_or_init(prometheus::Registry::new)
        .clone()
}

#[must_use]
pub fn prometheus_metrics_text() -> Option<Result<PrometheusMetricsText, String>>
{
    let registry = PROMETHEUS_REGISTRY.get()?.clone();
    let encoder = prometheus::TextEncoder::new();
    let metric_families = registry.gather();
    let mut body = Vec::new();
    Some(
        encoder
            .encode(&metric_families, &mut body)
            .map(|()| PrometheusMetricsText {
                content_type: encoder.format_type().to_string(),
                body,
            })
            .map_err(|error| error.to_string()),
    )
}

pub fn init_prometheus_metrics_for_current_process(
    service_name: impl Into<String>,
) -> Result<SdkMeterProvider, TelemetryError> {
    let resource = Resource::builder()
        .with_service_name(service_name.into())
        .build();
    let metrics_provider = prometheus_metrics_provider(resource)?;
    global::set_meter_provider(metrics_provider.clone());
    Ok(metrics_provider)
}

#[derive(Debug)]
pub struct UuidGenerator;

impl IdGenerator for UuidGenerator {
    fn new_trace_id(&self) -> opentelemetry::TraceId {
        TraceId::from(Uuid::new_v4().as_u128())
    }

    fn new_span_id(&self) -> opentelemetry::SpanId {
        opentelemetry::SpanId::from(Uuid::new_v4().as_u64_pair().0)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use opentelemetry::{
        KeyValue,
        trace::{Span, Tracer, TracerProvider},
    };
    use opentelemetry_sdk::{
        error::OTelSdkResult,
        trace::{SdkTracerProvider, SpanData, SpanExporter},
    };

    use super::{Config, MAX_ATTRIBUTES_PER_SPAN};

    const GATEWAY_ROUTE_SPAN_FIELDS: &[&str] = &[
        "router_id",
        "strategy",
        "agent_name",
        "work_unit_id",
        "work_unit_source",
        "client_subject_id",
        "client_key_id",
        "client_plan_id",
        "source_model",
        "candidates",
        "planned_hops",
        "plan_rebuilds",
        "route_memory_hit",
        "route_memory_invalidated",
        "json_schema_required",
        "duration_ms",
        "tfft_ms",
        "generation_ms_per_output_token",
        "input_tokens",
        "output_tokens",
        "usage_source",
        "terminal_provider",
        "terminal_credential",
        "terminal_model",
        "terminal_status",
        "terminal_outcome",
        "failure_stage",
        "error_source",
        "error_class",
        "response_body_bytes",
    ];

    #[derive(Clone, Debug, Default)]
    struct CapturingSpanExporter {
        spans: Arc<Mutex<Vec<SpanData>>>,
    }

    impl CapturingSpanExporter {
        fn finished_spans(&self) -> Vec<SpanData> {
            self.spans.lock().expect("finished spans").clone()
        }
    }

    impl SpanExporter for CapturingSpanExporter {
        async fn export(&self, mut batch: Vec<SpanData>) -> OTelSdkResult {
            self.spans
                .lock()
                .expect("finished spans")
                .append(&mut batch);
            Ok(())
        }
    }

    #[test]
    fn otlp_logs_default_enabled_for_backward_compatibility() {
        assert!(Config::default().otlp_logs);
    }

    #[test]
    fn span_attribute_budget_covers_gateway_route_fields() {
        assert!(
            MAX_ATTRIBUTES_PER_SPAN
                >= u32::try_from(GATEWAY_ROUTE_SPAN_FIELDS.len()).unwrap()
        );
        assert!(MAX_ATTRIBUTES_PER_SPAN >= 64);
    }

    #[test]
    fn exported_route_span_keeps_all_gateway_route_fields() {
        let exporter = CapturingSpanExporter::default();
        let provider = SdkTracerProvider::builder()
            .with_simple_exporter(exporter.clone())
            .with_id_generator(super::UuidGenerator)
            .with_max_events_per_span(super::MAX_EVENTS_PER_SPAN)
            .with_max_attributes_per_span(MAX_ATTRIBUTES_PER_SPAN)
            .build();
        let tracer = provider.tracer("telemetry-test");
        let mut span = tracer.start("gateway.route");

        for field in GATEWAY_ROUTE_SPAN_FIELDS {
            span.set_attribute(KeyValue::new(*field, "value"));
        }
        span.end();
        provider.force_flush().unwrap();

        let spans = exporter.finished_spans();
        let route_span = spans
            .iter()
            .find(|span| span.name == "gateway.route")
            .expect("exported gateway.route span");
        assert_eq!(route_span.dropped_attributes_count, 0);
        for field in GATEWAY_ROUTE_SPAN_FIELDS {
            assert!(
                route_span
                    .attributes
                    .iter()
                    .any(|attribute| attribute.key.as_str() == *field),
                "missing exported route span field {field}"
            );
        }
    }
}
