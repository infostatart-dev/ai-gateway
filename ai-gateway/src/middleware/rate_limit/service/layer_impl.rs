use std::sync::Arc;

use tower_governor::GovernorLayer;

use super::{InnerLayer, Layer};
use crate::{
    app_state::AppState,
    config::{
        rate_limit::{RateLimitConfig, RateLimitStore},
        router::RouterConfig,
    },
    error::init::InitError,
    middleware::rate_limit::redis_service::RedisRateLimitLayer,
    types::router::RouterId,
};

impl Layer {
    pub fn global(app_state: &AppState) -> Result<Self, InitError> {
        Self::from_config(
            app_state,
            app_state.config().global.rate_limit.as_ref(),
        )
    }

    pub fn unified_api(app_state: &AppState) -> Result<Self, InitError> {
        Self::from_config(
            app_state,
            app_state.config().unified_api.rate_limit.as_ref(),
        )
    }

    fn from_config(
        app_state: &AppState,
        config: Option<&RateLimitConfig>,
    ) -> Result<Self, InitError> {
        let config = match config {
            Some(c) => c,
            None => {
                return Ok(Self {
                    inner: InnerLayer::None,
                });
            }
        };
        let store =
            app_state.config().rate_limit_store.as_ref().ok_or(
                InitError::InvalidRateLimitConfig("store not configured"),
            )?;
        if let RateLimitStore::Redis(redis) = store {
            Ok(Self::new_redis_inner(
                config.limits.clone(),
                redis.host_url.expose().clone(),
            ))
        } else {
            Ok(Self::new_in_memory_inner(
                app_state.0.global_rate_limit.clone(),
            ))
        }
    }

    pub fn new_redis_inner(
        rl: crate::config::rate_limit::LimitsConfig,
        url: url::Url,
    ) -> Self {
        if let Ok(layer) = RedisRateLimitLayer::new(Arc::new(rl), url, None) {
            Self {
                inner: InnerLayer::Redis(layer),
            }
        } else {
            Self {
                inner: InnerLayer::None,
            }
        }
    }

    pub fn new_in_memory_inner(
        rl: Option<Arc<crate::config::rate_limit::RateLimiterConfig>>,
    ) -> Self {
        if let Some(rl) = rl {
            Self {
                inner: InnerLayer::InMemory(
                    GovernorLayer::new(rl).error_handler(
                        crate::config::rate_limit::rate_limit_error_handler,
                    ),
                ),
            }
        } else {
            Self {
                inner: InnerLayer::None,
            }
        }
    }

    pub fn disabled() -> Self {
        Self {
            inner: InnerLayer::None,
        }
    }

    pub async fn per_router(
        app_state: &AppState,
        router_id: RouterId,
        router_config: &RouterConfig,
    ) -> Result<Self, InitError> {
        match &router_config.rate_limit {
            None => Ok(Self {
                inner: InnerLayer::None,
            }),
            Some(RateLimitConfig { store, limits }) => {
                let store = store
                    .as_ref()
                    .or(app_state.config().rate_limit_store.as_ref())
                    .ok_or(InitError::InvalidRateLimitConfig(
                        "store not configured",
                    ))?;
                if let RateLimitStore::Redis(redis) = store {
                    if let Ok(layer) = RedisRateLimitLayer::new(
                        Arc::new(limits.clone()),
                        redis.host_url.expose().clone(),
                        Some(router_id.clone()),
                    ) {
                        return Ok(Self {
                            inner: InnerLayer::Redis(layer),
                        });
                    }
                }
                let rl = Arc::new(crate::config::rate_limit::limiter_config(
                    limits,
                )?);
                app_state
                    .0
                    .router_rate_limits
                    .write()
                    .await
                    .insert(router_id, rl.clone());
                Ok(Self {
                    inner: InnerLayer::InMemory(
                        GovernorLayer::new(rl).error_handler(
                            crate::config::rate_limit::rate_limit_error_handler,
                        ),
                    ),
                })
            }
        }
    }
}

impl<S> tower::layer::Layer<S> for Layer {
    type Service = super::Service<S>;
    fn layer(&self, service: S) -> Self::Service {
        match &self.inner {
            InnerLayer::InMemory(inner) => super::Service::InMemory {
                service: inner.layer(service),
            },
            InnerLayer::Redis(inner) => super::Service::Redis {
                service: inner.layer(service),
            },
            InnerLayer::None => super::Service::Disabled { service },
        }
    }
}
