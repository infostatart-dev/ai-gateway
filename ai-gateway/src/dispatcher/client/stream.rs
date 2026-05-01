use bytes::Bytes;
use futures::StreamExt;
use http_body_util::BodyExt;
use reqwest_eventsource::{Event, EventSource, RequestBuilderExt};
use tracing::{Instrument, info_span};

use super::metrics::record_stream_err_metrics;
use crate::{
    discover::monitor::metrics::EndpointMetricsRegistry,
    dispatcher::SSEStream,
    endpoints::ApiEndpoint,
    error::{api::ApiError, internal::InternalError, stream::StreamError},
};

impl super::Client {
    pub async fn sse_stream<B>(
        rb: reqwest::RequestBuilder,
        body: B,
        endpoint: Option<ApiEndpoint>,
        metrics: &EndpointMetricsRegistry,
    ) -> Result<SSEStream, ApiError>
    where
        B: Into<reqwest::Body>,
    {
        let es = rb
            .body(body)
            .eventsource()
            .map_err(|_| InternalError::Internal)?;
        sse_stream(es, endpoint, metrics.clone())
            .await
            .map_err(ApiError::StreamError)
    }
}

pub async fn sse_stream(
    mut es: EventSource,
    endpoint: Option<ApiEndpoint>,
    metrics: EndpointMetricsRegistry,
) -> Result<SSEStream, StreamError> {
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    if let Some(Ok(Event::Message(msg))) = es.next().await
        && msg.data != "[DONE]"
    {
        tx.send(Ok(Bytes::from(msg.data))).ok();
    } else if let Some(Err(e)) = es.next().await {
        handle_stream_error(e, endpoint.clone(), &metrics).await?;
    }
    tokio::spawn(
        async move {
            while let Some(ev) = es.next().await {
                match ev {
                    Err(e)
                        if matches!(
                            e,
                            reqwest_eventsource::Error::StreamEnded
                        ) =>
                    {
                        break;
                    }
                    Err(e) => {
                        if handle_stream_error_with_tx(
                            e,
                            tx.clone(),
                            endpoint.clone(),
                            &metrics,
                        )
                        .await
                        .is_err()
                        {
                            break;
                        }
                    }
                    Ok(Event::Message(msg)) if msg.data == "[DONE]" => break,
                    Ok(Event::Message(msg)) => {
                        if tx.send(Ok(Bytes::from(msg.data))).is_err() {
                            break;
                        }
                    }
                    _ => {}
                }
            }
            es.close();
        }
        .instrument(info_span!("sse_stream")),
    );
    Ok(Box::pin(
        tokio_stream::wrappers::UnboundedReceiverStream::new(rx),
    ))
}

async fn handle_stream_error_with_tx(
    e: reqwest_eventsource::Error,
    tx: tokio::sync::mpsc::UnboundedSender<Result<Bytes, ApiError>>,
    endpoint: Option<ApiEndpoint>,
    metrics: &EndpointMetricsRegistry,
) -> Result<(), InternalError> {
    record_stream_err_metrics(&e, endpoint, metrics);
    match e {
        reqwest_eventsource::Error::InvalidStatusCode(_s, r) => {
            let body = http::Response::from(r)
                .into_body()
                .collect()
                .await?
                .to_bytes();
            tx.send(Ok(body)).ok();
            Ok(())
        }
        e => {
            tx.send(Err(ApiError::StreamError(StreamError::StreamError(
                Box::new(e),
            ))))
            .ok();
            Ok(())
        }
    }
}

async fn handle_stream_error(
    e: reqwest_eventsource::Error,
    endpoint: Option<ApiEndpoint>,
    metrics: &EndpointMetricsRegistry,
) -> Result<(), StreamError> {
    record_stream_err_metrics(&e, endpoint, metrics);
    match e {
        reqwest_eventsource::Error::InvalidStatusCode(s, r) => {
            let body = http::Response::from(r)
                .into_body()
                .collect()
                .await
                .map_err(|e| {
                    StreamError::BodyError(axum_core::Error::new(
                        InternalError::ReqwestError(e),
                    ))
                })?
                .to_bytes();
            Err(StreamError::StreamError(Box::new(
                reqwest_eventsource::Error::InvalidStatusCode(
                    s,
                    http::Response::new(body).into(),
                ),
            )))
        }
        e => Err(StreamError::StreamError(Box::new(e))),
    }
}
