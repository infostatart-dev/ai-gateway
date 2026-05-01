use bytes::Bytes;
use futures::TryStreamExt;
use reqwest::RequestBuilder;

use super::Dispatcher;
use crate::{
    error::{api::ApiError, internal::InternalError},
    types::body::{Body, BodyReader},
};

impl Dispatcher {
    pub async fn dispatch_sync(
        request_builder: &RequestBuilder,
        req_body_bytes: Bytes,
    ) -> Result<
        (
            http::Response<Body>,
            BodyReader,
            tokio::sync::oneshot::Receiver<()>,
        ),
        ApiError,
    > {
        let request_builder = request_builder.try_clone().ok_or_else(|| {
            tracing::error!("failed to clone request builder");
            ApiError::Internal(InternalError::Internal)
        })?;
        let response = request_builder
            .body(req_body_bytes)
            .send()
            .await
            .map_err(InternalError::ReqwestError)?;
        let status = response.status();
        let mut resp_builder = http::Response::builder().status(status);
        *resp_builder.headers_mut().unwrap() = response.headers().clone();

        #[cfg(debug_assertions)]
        if status.is_server_error() || status.is_client_error() {
            let body =
                response.text().await.map_err(InternalError::ReqwestError)?;
            let stream =
                futures::stream::once(futures::future::ok::<_, ApiError>(
                    bytes::Bytes::from(body),
                ));
            let (error_body, error_reader, tfft_rx) =
                BodyReader::wrap_stream(stream, false);
            return Ok((
                resp_builder
                    .body(error_body)
                    .map_err(InternalError::HttpError)?,
                error_reader,
                tfft_rx,
            ));
        }

        let stream = response
            .bytes_stream()
            .map_err(|e| ApiError::Internal(InternalError::ReqwestError(e)));
        let (user_resp_body, body_reader, tfft_rx) =
            BodyReader::wrap_stream(stream, false);
        Ok((
            resp_builder
                .body(user_resp_body)
                .map_err(InternalError::HttpError)?,
            body_reader,
            tfft_rx,
        ))
    }
}
