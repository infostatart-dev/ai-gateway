use crate::{
    discover::monitor::metrics::EndpointMetricsRegistry, endpoints::ApiEndpoint,
};

pub fn record_stream_err_metrics(
    err: &reqwest_eventsource::Error,
    endpoint: Option<ApiEndpoint>,
    metrics: &EndpointMetricsRegistry,
) {
    if let Some(ep) = endpoint {
        metrics
            .health_metrics(ep)
            .map(|m| m.incr_for_stream_error(err))
            .ok();
    }
}
