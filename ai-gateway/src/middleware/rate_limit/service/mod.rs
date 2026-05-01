use governor::middleware::StateInformationMiddleware;
use tower_governor::GovernorLayer;

use crate::middleware::rate_limit::{
    extractor::RateLimitKeyExtractor,
    redis_service::{RedisRateLimitLayer, RedisRateLimitService},
};

pub mod future;
pub mod layer_impl;
pub mod service_impl;
#[cfg(all(test, feature = "testing"))]
pub mod tests;
pub mod utils;

pub type OptionalGovernorLayer = Option<
    GovernorLayer<
        RateLimitKeyExtractor,
        StateInformationMiddleware,
        crate::types::body::Body,
    >,
>;
pub type GovernorService<S> = tower_governor::governor::Governor<
    RateLimitKeyExtractor,
    StateInformationMiddleware,
    S,
    crate::types::body::Body,
>;

#[derive(Clone)]
pub enum InnerLayer {
    None,
    InMemory(
        GovernorLayer<
            RateLimitKeyExtractor,
            StateInformationMiddleware,
            crate::types::body::Body,
        >,
    ),
    Redis(RedisRateLimitLayer),
}

#[derive(Clone)]
pub struct Layer {
    pub inner: InnerLayer,
}

#[derive(Debug, Clone)]
pub enum Service<S> {
    Disabled { service: S },
    InMemory { service: GovernorService<S> },
    Redis { service: RedisRateLimitService<S> },
}
