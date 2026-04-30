use bytes::Bytes;
use http::StatusCode;
use reqwest::RequestBuilder;
use crate::{
    dispatcher::client::Client,
    discover::monitor::metrics::EndpointMetricsRegistry,
    endpoints::ApiEndpoint,
    error::{api::ApiError, internal::InternalError},
    types::body::{Body, BodyReader},
};
use super::{Dispatcher, retry::stream_response_headers};

impl Dispatcher {
    pub async fn dispatch_stream(request_builder: &RequestBuilder, req_body_bytes: Bytes, api_endpoint: Option<ApiEndpoint>, metrics_registry: EndpointMetricsRegistry) -> Result<(http::Response<Body>, BodyReader, tokio::sync::oneshot::Receiver<()>), ApiError> {
        let request_builder = request_builder.try_clone().ok_or_else(|| { tracing::error!("failed to clone request builder"); ApiError::Internal(InternalError::Internal) })?;
        let response_stream = Client::sse_stream(request_builder, req_body_bytes, api_endpoint, &metrics_registry).await?;
        let mut resp_builder = http::Response::builder();
        *resp_builder.headers_mut().unwrap() = stream_response_headers();
        let (user_resp_body, body_reader, tfft_rx) = BodyReader::wrap_stream(response_stream, true);
        Ok((resp_builder.status(StatusCode::OK).body(user_resp_body).map_err(InternalError::HttpError)?, body_reader, tfft_rx))
    }
}
