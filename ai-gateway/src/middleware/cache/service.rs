use super::{
    context::{CacheContext, get_cache_ctx},
    request::make_request,
};
use crate::{
    app_state::AppState,
    cache::CacheClient,
    config::{cache::CacheConfig, router::RouterConfig},
    error::{api::ApiError, init::InitError, internal::InternalError},
    types::{request::Request, response::Response},
};
use futures::future::BoxFuture;
use std::{
    convert::Infallible,
    sync::Arc,
    task::{Context, Poll},
};

#[derive(Debug, Clone)]
pub struct CacheLayer {
    app_state: AppState,
    backend: CacheClient,
    context: Arc<CacheContext>,
}

impl CacheLayer {
    fn new(
        app_state: AppState,
        config: CacheConfig,
    ) -> Result<Self, InitError> {
        let backend = app_state
            .0
            .cache_manager
            .clone()
            .ok_or(InitError::CacheNotConfigured)?;
        let context = CacheContext {
            enabled: Some(true),
            directive: config.directive,
            buckets: Some(config.buckets),
            seed: config.seed,
            options: Some(http_cache_semantics::CacheOptions {
                shared: false,
                ..Default::default()
            }),
        };
        Ok(Self {
            app_state,
            backend,
            context: Arc::new(context),
        })
    }

    pub fn for_router(
        app_state: &AppState,
        router_config: &RouterConfig,
    ) -> Option<Self> {
        router_config.cache.as_ref().and_then(|config| {
            Self::new(app_state.clone(), config.clone()).ok()
        })
    }

    pub fn global(app_state: &AppState) -> Result<Option<Self>, InitError> {
        app_state
            .config()
            .global
            .cache
            .as_ref()
            .map(|config| Self::new(app_state.clone(), config.clone()))
            .transpose()
    }

    pub fn unified_api(
        app_state: &AppState,
    ) -> Result<Option<Self>, InitError> {
        app_state
            .config()
            .unified_api
            .cache
            .as_ref()
            .map(|config| Self::new(app_state.clone(), config.clone()))
            .transpose()
    }
}

impl<S> tower::Layer<S> for CacheLayer {
    type Service = CacheService<S>;
    fn layer(&self, inner: S) -> Self::Service {
        CacheService {
            inner,
            app_state: self.app_state.clone(),
            backend: self.backend.clone(),
            context: Arc::clone(&self.context),
        }
    }
}

#[derive(Debug, Clone)]
pub struct CacheService<S> {
    inner: S,
    app_state: AppState,
    backend: CacheClient,
    context: Arc<CacheContext>,
}

impl<S> tower::Service<Request> for CacheService<S>
where
    S: tower::Service<Request, Response = Response, Error = Infallible>
        + Send
        + Clone
        + 'static,
    S::Future: Send + 'static,
{
    type Response = Response;
    type Error = ApiError;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(
        &mut self,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), Self::Error>> {
        self.inner
            .poll_ready(cx)
            .map_err(|_| ApiError::Internal(InternalError::Internal))
    }

    #[tracing::instrument(name = "cache", skip_all)]
    fn call(&mut self, req: Request) -> Self::Future {
        let mut this = self.clone();
        std::mem::swap(self, &mut this);
        Box::pin(async move {
            let merged_ctx = this.context.merge(&get_cache_ctx(&req)?);
            let backend = this.backend.clone();
            make_request(
                &mut this.inner,
                &this.app_state,
                req,
                &backend,
                merged_ctx,
            )
            .await
        })
    }
}
