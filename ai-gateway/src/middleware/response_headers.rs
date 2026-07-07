use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};

use futures::ready;
use http::{Request, Response};
use pin_project_lite::pin_project;

use crate::{
    config::{
        observability::ObservabilityResponseHeadersConfig,
        response_headers::ResponseHeadersConfig,
    },
    types::{
        extensions::{
            CallerRequestContext, ProviderRequestId, RoutedModelAndProvider,
            WorkUnitSource,
        },
        provider::InferenceProvider,
    },
};

#[derive(Debug, Clone)]
pub struct ResponseHeaderService<S> {
    config: ResponseHeadersConfig,
    observability: ObservabilityResponseHeadersConfig,
    inner: S,
}

impl<S> ResponseHeaderService<S> {
    pub const fn new(
        config: ResponseHeadersConfig,
        observability: ObservabilityResponseHeadersConfig,
        inner: S,
    ) -> ResponseHeaderService<S> {
        ResponseHeaderService {
            config,
            observability,
            inner,
        }
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
            observability: self.observability,
            inner: self.inner.call(req),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ResponseHeaderLayer {
    config: ResponseHeadersConfig,
    observability: ObservabilityResponseHeadersConfig,
}

impl ResponseHeaderLayer {
    #[must_use]
    pub const fn new(
        config: ResponseHeadersConfig,
        observability: ObservabilityResponseHeadersConfig,
    ) -> Self {
        Self {
            config,
            observability,
        }
    }
}

impl<S> tower::Layer<S> for ResponseHeaderLayer {
    type Service = ResponseHeaderService<S>;

    fn layer(&self, service: S) -> ResponseHeaderService<S> {
        ResponseHeaderService::new(self.config, self.observability, service)
    }
}

pin_project! {
    pub struct ResponseFuture<F> {
        config: ResponseHeadersConfig,
        observability: ObservabilityResponseHeadersConfig,
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
        && !response.headers().contains_key(
            http::HeaderName::from_static(
                crate::metrics::provider::usage_json::GATEWAY_PROVIDER_USAGE_HEADER,
            ),
        ) {
            response.headers_mut().insert(
                http::HeaderName::from_static(
                    crate::metrics::provider::usage_json::GATEWAY_PROVIDER_USAGE_HEADER,
                ),
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

        apply_declared_model_headers(&mut response);

        if this.observability.echo_work_unit_id
            && let Some(caller) =
                response.extensions().get::<CallerRequestContext>()
            && matches!(
                caller.work_unit_source,
                WorkUnitSource::RequestId | WorkUnitSource::Generated
            )
            && let Some(work_unit_id) = caller.work_unit_id.as_deref()
            && let Ok(header_value) = http::HeaderValue::from_str(work_unit_id)
        {
            response.headers_mut().insert(
                http::HeaderName::from_static("x-work-unit-id"),
                header_value,
            );
        }
        Poll::Ready(Ok(response))
    }
}

fn apply_declared_model_headers<RespBody>(response: &mut Response<RespBody>) {
    let source_model = response
        .extensions()
        .get::<crate::types::extensions::PendingRouteTrace>()
        .and_then(|pending| pending.source_model.as_deref())
        .map(str::to_owned);
    if let Some(source_model) = source_model.as_deref() {
        crate::declared_models::apply_smart_headers(
            response.headers_mut(),
            source_model,
        );
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

    fn pending_route_trace(
        source_model: &'static str,
    ) -> crate::types::extensions::PendingRouteTrace {
        crate::types::extensions::PendingRouteTrace {
            router_id: crate::types::router::RouterId::Named("test".into()),
            strategy: "test",
            hops: 1,
            candidates: 1,
            skipped: 0,
            outcome_label: "success",
            terminal_provider: None,
            terminal_credential: None,
            terminal_status: Some(200),
            deepseek_web: None,
            chatgpt_web: None,
            intent_tier: None,
            selection_phase: None,
            quota_scope: None,
            model_ladder_band: None,
            model_ladder_position: None,
            upstream_failure_kind: None,
            restricted_until: None,
            failover_class: None,
            failure_stage: None,
            error_source: None,
            error_class: None,
            agent_name: None,
            work_unit_id: None,
            work_unit_source: None,
            planned_hops: None,
            plan_rebuilds: None,
            route_memory_hit: None,
            route_memory_invalidated: None,
            summary: crate::types::extensions::RouteTraceSummary::default(),
            source_model: Some(source_model.to_string()),
            json_schema_required: false,
            estimated_usage: crate::metrics::llm::TokenUsage::default(),
            replay: None,
            finalize: None,
        }
    }

    #[tokio::test]
    async fn test_response_headers_disabled() {
        let config = ResponseHeadersConfig {
            provider: false,
            provider_request_id: false,
        };

        let mut service = ResponseHeaderService::new(
            config,
            ObservabilityResponseHeadersConfig::default(),
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
    async fn stable_declared_model_gets_smart_status_header() {
        let config = ResponseHeadersConfig {
            provider: false,
            provider_request_id: false,
        };

        let mut service = ResponseHeaderService::new(
            config,
            ObservabilityResponseHeadersConfig::default(),
            create_mock_service(|| {
                let mut response = Response::new("test".to_string());
                response
                    .extensions_mut()
                    .insert(pending_route_trace("gpt-5.5-mini"));
                response
            }),
        );

        let request = Request::new(());
        let response =
            service.ready().await.unwrap().call(request).await.unwrap();

        assert_eq!(
            response
                .headers()
                .get(crate::declared_models::SMART_STATUS_HEADER)
                .unwrap(),
            crate::declared_models::STABLE_BINDING_STATUS
        );
        assert!(
            !response
                .headers()
                .contains_key(crate::declared_models::SMART_WARNING_HEADER)
        );
    }

    #[tokio::test]
    async fn unstable_declared_model_gets_smart_warning_header() {
        let config = ResponseHeadersConfig {
            provider: false,
            provider_request_id: false,
        };

        let mut service = ResponseHeaderService::new(
            config,
            ObservabilityResponseHeadersConfig::default(),
            create_mock_service(|| {
                let mut response = Response::new("test".to_string());
                response
                    .extensions_mut()
                    .insert(pending_route_trace("gpt-5.5"));
                response
            }),
        );

        let request = Request::new(());
        let response =
            service.ready().await.unwrap().call(request).await.unwrap();

        assert_eq!(
            response
                .headers()
                .get(crate::declared_models::SMART_WARNING_HEADER)
                .unwrap(),
            crate::declared_models::UNSTABLE_GPT55_WARNING
        );
        assert!(
            !response
                .headers()
                .contains_key(crate::declared_models::SMART_STATUS_HEADER)
        );
    }

    #[tokio::test]
    async fn test_provider_header_enabled() {
        let config = ResponseHeadersConfig {
            provider: true,
            provider_request_id: false,
        };

        let mut service = ResponseHeaderService::new(
            config,
            ObservabilityResponseHeadersConfig::default(),
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
            ObservabilityResponseHeadersConfig::default(),
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
            ObservabilityResponseHeadersConfig::default(),
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
            ObservabilityResponseHeadersConfig::default(),
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
            ObservabilityResponseHeadersConfig::default(),
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
                            ttfb: None,
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
            ObservabilityResponseHeadersConfig::default(),
            create_mock_service(|| Response::new("test".to_string())),
        );

        let request = Request::new(());
        let response =
            service.ready().await.unwrap().call(request).await.unwrap();

        assert!(!response.headers().contains_key("helicone-provider-req-id"));
    }
}
