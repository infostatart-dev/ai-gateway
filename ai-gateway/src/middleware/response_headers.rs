use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};

use futures::ready;
use http::{Request, Response};
use pin_project_lite::pin_project;

use crate::{
    config::response_headers::ResponseHeadersConfig,
    types::{
        extensions::{ProviderRequestId, RoutedModelAndProvider},
        provider::InferenceProvider,
    },
};

#[derive(Debug, Clone)]
pub struct ResponseHeaderService<S> {
    config: ResponseHeadersConfig,
    inner: S,
}

impl<S> ResponseHeaderService<S> {
    pub const fn new(
        config: ResponseHeadersConfig,
        inner: S,
    ) -> ResponseHeaderService<S> {
        ResponseHeaderService { config, inner }
    }
}

impl<S, ReqBody, RespBody> tower::Service<Request<ReqBody>>
    for ResponseHeaderService<S>
where
    S: tower::Service<Request<ReqBody>, Response = Response<RespBody>>
        + Send
        + 'static,
    S::Future: Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = ResponseFuture<S::Future>;

    #[inline]
    fn poll_ready(
        &mut self,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request<ReqBody>) -> Self::Future {
        ResponseFuture {
            config: self.config,
            inner: self.inner.call(req),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ResponseHeaderLayer(ResponseHeadersConfig);

impl ResponseHeaderLayer {
    #[must_use]
    pub const fn new(config: ResponseHeadersConfig) -> Self {
        Self(config)
    }
}

impl<S> tower::Layer<S> for ResponseHeaderLayer {
    type Service = ResponseHeaderService<S>;

    fn layer(&self, service: S) -> ResponseHeaderService<S> {
        ResponseHeaderService::new(self.0, service)
    }
}

pin_project! {
    pub struct ResponseFuture<F> {
        config: ResponseHeadersConfig,
        #[pin]
        inner: F,
    }
}

impl<F, RespBody, E> Future for ResponseFuture<F>
where
    F: Future<Output = Result<Response<RespBody>, E>>,
{
    type Output = F::Output;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();
        let mut response = match ready!(this.inner.poll(cx)) {
            Ok(response) => response,
            Err(e) => {
                return Poll::Ready(Err(e));
            }
        };
        if this.config.provider {
            let inference_provider =
                response.extensions().get::<InferenceProvider>();
            if let Some(inference_provider) = inference_provider
                && let Ok(header_value) =
                    http::HeaderValue::from_str(inference_provider.as_ref())
            {
                response
                    .headers_mut()
                    .insert("helicone-provider", header_value);
            }
        }

        if this.config.provider_request_id {
            let provider_request_id =
                response.extensions().get::<ProviderRequestId>().cloned();
            if let Some(provider_request_id) = provider_request_id {
                response
                    .headers_mut()
                    .insert("helicone-provider-req-id", provider_request_id.0);
            }
        }

        if let Some(usage) =
            response.extensions().get::<crate::types::extensions::GatewayProviderUsageExtension>()
        && let Some(header_value) = usage.0.to_header_value()
        {
            response.headers_mut().insert(
                http::HeaderName::from_static("x-gateway-provider-usage"),
                header_value,
            );
        }

        if let Some(routed) =
            response.extensions().get::<RoutedModelAndProvider>()
            && let Ok(header_value) = http::HeaderValue::from_str(&routed.0)
        {
            response.headers_mut().insert(
                http::HeaderName::from_static("x-realmode-model-and-provider"),
                header_value,
            );
        }

        if let Some(intent) = response
            .extensions()
            .get::<crate::types::extensions::RoutingIntentContext>()
            .copied()
        {
            if let Ok(header_value) =
                http::HeaderValue::from_str(intent.intent_tier.as_str())
            {
                response.headers_mut().insert(
                    http::HeaderName::from_static("x-routing-intent-tier"),
                    header_value,
                );
            }
            if let Ok(header_value) =
                http::HeaderValue::from_str(intent.selection_phase.as_str())
            {
                response.headers_mut().insert(
                    http::HeaderName::from_static("x-routing-selection-phase"),
                    header_value,
                );
            }
        }
        Poll::Ready(Ok(response))
    }
}

#[cfg(test)]
mod tests {
    use std::convert::Infallible;

    use http::HeaderValue;
    use tower::{Service, ServiceExt, service_fn};

    use super::*;

    fn create_mock_service<F>(
        response_fn: F,
    ) -> impl tower::Service<
        Request<()>,
        Response = Response<String>,
        Error = Infallible,
        Future = std::future::Ready<Result<Response<String>, Infallible>>,
    >
    where
        F: Fn() -> Response<String> + Clone,
    {
        service_fn(move |_req| {
            let response_fn = response_fn.clone();
            std::future::ready(Ok(response_fn()))
        })
    }

    #[tokio::test]
    async fn test_response_headers_disabled() {
        let config = ResponseHeadersConfig {
            provider: false,
            provider_request_id: false,
        };

        let mut service = ResponseHeaderService::new(
            config,
            create_mock_service(|| {
                let mut response = Response::new("test".to_string());
                response.extensions_mut().insert(InferenceProvider::OpenAI);
                response.extensions_mut().insert(ProviderRequestId(
                    HeaderValue::from_static("test-req-id"),
                ));
                response
            }),
        );

        let request = Request::new(());
        let response =
            service.ready().await.unwrap().call(request).await.unwrap();

        assert!(!response.headers().contains_key("helicone-provider"));
        assert!(!response.headers().contains_key("helicone-provider-req-id"));
    }

    #[tokio::test]
    async fn test_provider_header_enabled() {
        let config = ResponseHeadersConfig {
            provider: true,
            provider_request_id: false,
        };

        let mut service = ResponseHeaderService::new(
            config,
            create_mock_service(|| {
                let mut response = Response::new("test".to_string());
                response
                    .extensions_mut()
                    .insert(InferenceProvider::Anthropic);
                response
            }),
        );

        let request = Request::new(());
        let response =
            service.ready().await.unwrap().call(request).await.unwrap();

        assert_eq!(
            response.headers().get("helicone-provider").unwrap(),
            "anthropic"
        );
        assert!(!response.headers().contains_key("helicone-provider-req-id"));
    }

    #[tokio::test]
    async fn test_provider_request_id_header_enabled() {
        let config = ResponseHeadersConfig {
            provider: false,
            provider_request_id: true,
        };

        let mut service = ResponseHeaderService::new(
            config,
            create_mock_service(|| {
                let mut response = Response::new("test".to_string());
                response.extensions_mut().insert(ProviderRequestId(
                    HeaderValue::from_static("req-123"),
                ));
                response
            }),
        );

        let request = Request::new(());
        let response =
            service.ready().await.unwrap().call(request).await.unwrap();

        assert!(!response.headers().contains_key("helicone-provider"));
        assert_eq!(
            response.headers().get("helicone-provider-req-id").unwrap(),
            "req-123"
        );
    }

    #[tokio::test]
    async fn test_both_headers_enabled() {
        let config = ResponseHeadersConfig {
            provider: true,
            provider_request_id: true,
        };

        let mut service = ResponseHeaderService::new(
            config,
            create_mock_service(|| {
                let mut response = Response::new("test".to_string());
                response
                    .extensions_mut()
                    .insert(InferenceProvider::GoogleGemini);
                response.extensions_mut().insert(ProviderRequestId(
                    HeaderValue::from_static("google-req-456"),
                ));
                response
            }),
        );

        let request = Request::new(());
        let response =
            service.ready().await.unwrap().call(request).await.unwrap();

        assert_eq!(
            response.headers().get("helicone-provider").unwrap(),
            "gemini"
        );
        assert_eq!(
            response.headers().get("helicone-provider-req-id").unwrap(),
            "google-req-456"
        );
    }

    #[tokio::test]
    async fn test_missing_provider_extension() {
        let config = ResponseHeadersConfig {
            provider: true,
            provider_request_id: false,
        };

        let mut service = ResponseHeaderService::new(
            config,
            create_mock_service(|| Response::new("test".to_string())),
        );

        let request = Request::new(());
        let response =
            service.ready().await.unwrap().call(request).await.unwrap();

        assert!(!response.headers().contains_key("helicone-provider"));
    }

    #[tokio::test]
    async fn test_gateway_provider_usage_header() {
        use crate::{
            metrics::provider::usage_json::{
                GatewayProviderUsage, LatencyBlock, RoutingBlock, UsageBlock,
            },
            types::extensions::GatewayProviderUsageExtension,
        };

        let config = ResponseHeadersConfig {
            provider: false,
            provider_request_id: false,
        };

        let mut service = ResponseHeaderService::new(
            config,
            create_mock_service(|| {
                let mut response = Response::new("test".to_string());
                response.extensions_mut().insert(
                    GatewayProviderUsageExtension(GatewayProviderUsage {
                        provider: "groq".to_string(),
                        credential: Some("default".to_string()),
                        model: None,
                        usage: UsageBlock {
                            input: Some(1),
                            output: Some(2),
                            cached: None,
                            reasoning: None,
                            total: Some(3),
                            source: "estimated",
                        },
                        latency_ms: LatencyBlock {
                            total: 50.0,
                            ttft: None,
                            generation_per_output_token: None,
                        },
                        routing: RoutingBlock {
                            attempts: 1,
                            failover: false,
                        },
                    }),
                );
                response
            }),
        );

        let request = Request::new(());
        let response =
            service.ready().await.unwrap().call(request).await.unwrap();

        let header = response
            .headers()
            .get("x-gateway-provider-usage")
            .expect("usage header");
        let parsed: serde_json::Value =
            serde_json::from_str(header.to_str().unwrap()).unwrap();
        assert_eq!(parsed["usage"]["source"], "estimated");
    }

    #[tokio::test]
    async fn test_missing_provider_request_id_extension() {
        let config = ResponseHeadersConfig {
            provider: false,
            provider_request_id: true,
        };

        let mut service = ResponseHeaderService::new(
            config,
            create_mock_service(|| Response::new("test".to_string())),
        );

        let request = Request::new(());
        let response =
            service.ready().await.unwrap().call(request).await.unwrap();

        assert!(!response.headers().contains_key("helicone-provider-req-id"));
    }
}
