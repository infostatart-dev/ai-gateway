use std::{
    str::FromStr,
    task::{Context, Poll},
};

use bytes::{BufMut, Bytes, BytesMut};
use futures::{TryStreamExt, future::BoxFuture};
use http::uri::PathAndQuery;
use tracing::{Instrument, info_span};

use crate::{
    endpoints::ApiEndpoint,
    error::{
        api::{ApiError, ErrorDetails, ErrorResponse},
        internal::InternalError,
        mapper::MapperError,
        stream::StreamError,
    },
    middleware::mapper::registry::EndpointConverterRegistry,
    types::{
        extensions::MapperContext, provider::InferenceProvider,
        request::Request, response::Response,
    },
};

#[derive(Debug, Clone)]
pub struct Service<S> {
    inner: S,
    endpoint_converter_registry: EndpointConverterRegistry,
}

impl<S> Service<S> {
    pub fn new(
        inner: S,
        endpoint_converter_registry: EndpointConverterRegistry,
    ) -> Self {
        Self {
            inner,
            endpoint_converter_registry,
        }
    }
}

impl<S> tower::Service<Request> for Service<S>
where
    S: tower::Service<
            Request,
            Response = http::Response<crate::types::body::Body>,
            Error = ApiError,
        > + Clone
        + Send
        + 'static,
    S::Future: Send + 'static,
{
    type Response = Response;
    type Error = ApiError;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    #[inline]
    fn poll_ready(
        &mut self,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    #[tracing::instrument(name = "mapper", skip_all)]
    fn call(&mut self, mut req: Request) -> Self::Future {
        // see: https://docs.rs/tower/latest/tower/trait.Service.html#be-careful-when-cloning-inner-services
        let mut inner = self.inner.clone();
        let converter_registry = self.endpoint_converter_registry.clone();
        std::mem::swap(&mut self.inner, &mut inner);
        Box::pin(async move {
            let target_provider = req
                .extensions()
                .get::<InferenceProvider>()
                .cloned()
                .ok_or_else(|| {
                    ApiError::Internal(InternalError::ExtensionNotFound(
                        "InferenceProvider",
                    ))
                })?;
            let extracted_path_and_query = req
                .extensions_mut()
                .remove::<PathAndQuery>()
                .ok_or(ApiError::Internal(InternalError::ExtensionNotFound(
                    "PathAndQuery",
                )))?;
            let source_endpoint =
                req.extensions().get::<ApiEndpoint>().cloned();
            let source_endpoint = source_endpoint.ok_or(ApiError::Internal(
                InternalError::ExtensionNotFound("ApiEndpoint"),
            ))?;
            let source_endpoint_cloned = source_endpoint.clone();
            let target_endpoint =
                ApiEndpoint::mapped(source_endpoint, &target_provider)?;
            let target_endpoint_cloned = target_endpoint.clone();
            // serialization/deserialization should be done on a dedicated
            // thread
            let converter_registry_cloned = converter_registry.clone();
            let source_endpoint_for_req = source_endpoint_cloned.clone();
            let target_endpoint_for_req = target_endpoint_cloned.clone();
            let req = tokio::task::spawn_blocking(move || async move {
                map_request(
                    converter_registry_cloned,
                    source_endpoint_for_req,
                    target_endpoint_for_req,
                    &extracted_path_and_query,
                    req,
                )
                .instrument(info_span!("map_request"))
                .await
            })
            .await
            .map_err(InternalError::MappingTaskError)?
            .await?;
            let response = inner.call(req).await?;
            let response = tokio::task::spawn_blocking(move || async move {
                map_response(
                    converter_registry,
                    target_endpoint_cloned,
                    source_endpoint_cloned,
                    response,
                )
                .await
            })
            .instrument(info_span!("map_response"))
            .await
            .map_err(InternalError::MappingTaskError)?
            .await?;
            Ok(response)
        })
    }
}

async fn map_request(
    converter_registry: EndpointConverterRegistry,
    source_endpoint: ApiEndpoint,
    target_endpoint: ApiEndpoint,
    target_path_and_query: &PathAndQuery,
    req: Request,
) -> Result<Request, ApiError> {
    use http_body_util::BodyExt;
    let (parts, body) = req.into_parts();
    let body = body
        .collect()
        .await
        .map_err(InternalError::CollectBodyError)?
        .to_bytes();
    let converter = converter_registry
        .get_converter(&source_endpoint, &target_endpoint)
        .ok_or_else(|| {
            InternalError::InvalidConverter(
                source_endpoint.clone(),
                target_endpoint.clone(),
            )
        })?;

    let (body, mapper_ctx) = converter.convert_req_body(body)?;
    let base_path = target_endpoint
        .path(mapper_ctx.model.as_ref(), mapper_ctx.is_stream)?;

    let target_path_and_query =
        if let Some(query_params) = target_path_and_query.query() {
            format!("{base_path}?{query_params}")
        } else {
            base_path
        };
    let target_path_and_query = PathAndQuery::from_str(&target_path_and_query)
        .map_err(InternalError::InvalidUri)?;

    let mut req = Request::from_parts(parts, axum_core::body::Body::from(body));
    tracing::trace!(
        source_endpoint = ?source_endpoint,
        target_endpoint = ?target_endpoint,
        target_path_and_query = ?target_path_and_query,
        mapper_ctx = ?mapper_ctx,
        "mapped request"
    );
    req.extensions_mut().insert(target_path_and_query);
    req.extensions_mut().insert(mapper_ctx);
    req.extensions_mut().insert(target_endpoint);
    Ok(req)
}

async fn map_response(
    converter_registry: EndpointConverterRegistry,
    source_endpoint: ApiEndpoint,
    target_endpoint: ApiEndpoint,
    resp: http::Response<crate::types::body::Body>,
) -> Result<Response, ApiError> {
    let mapper_ctx = resp
        .extensions()
        .get::<MapperContext>()
        .ok_or(InternalError::ExtensionNotFound("MapperContext"))?;
    let is_stream = mapper_ctx.is_stream;
    let (parts, body) = resp.into_parts();

    if is_stream {
        Ok(map_streaming_response(
            &converter_registry,
            &source_endpoint,
            &target_endpoint,
            parts,
            body,
        ))
    } else {
        map_non_streaming_response(
            converter_registry,
            source_endpoint,
            target_endpoint,
            parts,
            body,
        )
        .await
    }
}

fn map_streaming_response(
    converter_registry: &EndpointConverterRegistry,
    source_endpoint: &ApiEndpoint,
    target_endpoint: &ApiEndpoint,
    mut parts: http::response::Parts,
    body: crate::types::body::Body,
) -> Response {
    tracing::trace!(
        source_endpoint = ?target_endpoint,
        target_endpoint = ?source_endpoint,
        "mapped streaming response"
    );
    // The dispatcher constructs this body from either an SSE stream or a byte
    // stream, so each frame is treated as a single SSE event here.
    let mapped_stream = body
        .into_data_stream()
        .map_err(|e| ApiError::StreamError(StreamError::BodyError(e)))
        .try_filter_map({
            let captured_registry = converter_registry.clone();
            let resp_parts = parts.clone();
            let target_endpoint_cloned = target_endpoint.clone();
            let source_endpoint_cloned = source_endpoint.clone();
            move |bytes| {
                let registry_for_future = captured_registry.clone();
                let resp_parts = resp_parts.clone();
                let target_endpoint = target_endpoint_cloned.clone();
                let source_endpoint = source_endpoint_cloned.clone();
                async move {
                    let converter = registry_for_future
                        .get_converter(&target_endpoint, &source_endpoint)
                        .ok_or_else(|| {
                            InternalError::InvalidConverter(
                                target_endpoint.clone(),
                                source_endpoint.clone(),
                            )
                        })?;

                    let converted_data =
                        converter.convert_resp_body(resp_parts, bytes, true)?;

                    if let Some(converted_data) = converted_data {
                        let mut new_bytes = BytesMut::new();
                        new_bytes.put("data: ".as_bytes());
                        new_bytes.put(converted_data);
                        new_bytes.put("\n\n".as_bytes());
                        Ok(Some(new_bytes.freeze()))
                    } else {
                        Ok(converted_data)
                    }
                }
            }
        });
    let final_body =
        axum_core::body::Body::new(reqwest::Body::wrap_stream(mapped_stream));
    parts.headers.remove(http::header::CONTENT_LENGTH);
    Response::from_parts(parts, final_body)
}

async fn map_non_streaming_response(
    converter_registry: EndpointConverterRegistry,
    source_endpoint: ApiEndpoint,
    target_endpoint: ApiEndpoint,
    mut parts: http::response::Parts,
    body: crate::types::body::Body,
) -> Result<Response, ApiError> {
    let converter = converter_registry
        .get_converter(&target_endpoint, &source_endpoint)
        .ok_or_else(|| {
            InternalError::InvalidConverter(
                target_endpoint.clone(),
                source_endpoint.clone(),
            )
        })?;
    let body_bytes = http_body_util::BodyExt::collect(body)
        .await
        .map_err(InternalError::CollectBodyError)?
        .to_bytes();

    let (mapped_body_bytes, used_error_fallback) = match converter
        .convert_resp_body(parts.clone(), body_bytes.clone(), false)
    {
        Ok(Some(bytes)) => (bytes, false),
        Ok(None) => {
            return Err(InternalError::MapperError(
                MapperError::EmptyResponseBody,
            )
            .into());
        }
        Err(error)
            if parts.status.is_client_error()
                || parts.status.is_server_error() =>
        {
            tracing::warn!(
                status = %parts.status,
                error = %error,
                "failed to map upstream error response; returning generic OpenAI-compatible error"
            );
            (
                generic_upstream_error_body(parts.status, &body_bytes)?,
                true,
            )
        }
        Err(error) => return Err(error),
    };
    let final_body = axum_core::body::Body::from(mapped_body_bytes);
    parts.headers.remove(http::header::CONTENT_LENGTH);
    if used_error_fallback {
        parts.headers.insert(
            http::header::CONTENT_TYPE,
            http::HeaderValue::from_static("application/json"),
        );
    }
    tracing::trace!(
        source_endpoint = ?target_endpoint,
        target_endpoint = ?source_endpoint,
        "mapped non-streaming response"
    );
    Ok(Response::from_parts(parts, final_body))
}

fn generic_upstream_error_body(
    status: http::StatusCode,
    body_bytes: &Bytes,
) -> Result<Bytes, ApiError> {
    let raw_message = String::from_utf8_lossy(body_bytes);
    let message = if raw_message.trim().is_empty() {
        format!("Upstream returned {status}")
    } else {
        raw_message.trim().to_string()
    };
    let error_type = if status == http::StatusCode::TOO_MANY_REQUESTS {
        "rate_limit_exceeded"
    } else if status == http::StatusCode::UNAUTHORIZED {
        "authentication_error"
    } else if status == http::StatusCode::FORBIDDEN {
        "forbidden"
    } else if status.is_client_error() {
        "invalid_request_error"
    } else {
        "server_error"
    };
    serde_json::to_vec(&ErrorResponse {
        error: ErrorDetails {
            message,
            r#type: Some(error_type.to_string()),
            param: None,
            code: None,
        },
    })
    .map(Bytes::from)
    .map_err(|error| {
        InternalError::Serialize {
            ty: "ErrorResponse",
            error,
        }
        .into()
    })
}

#[derive(Debug, Clone)]
pub struct Layer {
    endpoint_converter_registry: EndpointConverterRegistry,
}

impl Layer {
    #[must_use]
    pub fn new(endpoint_converter_registry: EndpointConverterRegistry) -> Self {
        Self {
            endpoint_converter_registry,
        }
    }
}

impl<S> tower::Layer<S> for Layer {
    type Service = Service<S>;

    fn layer(&self, inner: S) -> Self::Service {
        Service::new(inner, self.endpoint_converter_registry.clone())
    }
}

#[cfg(test)]
mod tests {
    use bytes::Bytes;
    use http::StatusCode;

    use super::generic_upstream_error_body;
    use crate::error::api::ErrorResponse;

    #[test]
    fn generic_upstream_error_body_wraps_plaintext_rate_limit() {
        let body = generic_upstream_error_body(
            StatusCode::TOO_MANY_REQUESTS,
            &Bytes::from_static(b"Too many requests"),
        )
        .expect("fallback body");

        let response: ErrorResponse =
            serde_json::from_slice(&body).expect("json error");
        assert_eq!(response.error.message, "Too many requests");
        assert_eq!(
            response.error.r#type.as_deref(),
            Some("rate_limit_exceeded")
        );
    }

    #[test]
    fn generic_upstream_error_body_handles_empty_body() {
        let body =
            generic_upstream_error_body(StatusCode::BAD_GATEWAY, &Bytes::new())
                .expect("fallback body");

        let response: ErrorResponse =
            serde_json::from_slice(&body).expect("json error");
        assert_eq!(response.error.message, "Upstream returned 502 Bad Gateway");
        assert_eq!(response.error.r#type.as_deref(), Some("server_error"));
    }
}
