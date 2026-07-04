use crate::config::Config;

#[cfg(debug_assertions)]
pub fn apply_dev_rate_limit_overrides(config: &mut Config) {
    config.global.rate_limit = None;
    config.unified_api.rate_limit = None;
    for router in config.routers.as_mut().values_mut() {
        router.rate_limit = None;
    }
}

#[cfg(not(debug_assertions))]
pub fn apply_dev_rate_limit_overrides(_config: &mut Config) {}

#[cfg(test)]
mod tests {
    use compact_str::CompactString;

    use super::*;
    use crate::{
        config::{
            rate_limit::RateLimitConfig,
            router::{RouterConfig, RouterConfigs},
        },
        types::router::RouterId,
    };

    #[test]
    fn debug_override_disables_http_rate_limits() {
        let mut config = Config::default();
        config.global.rate_limit = Some(RateLimitConfig::default());
        config.unified_api.rate_limit = Some(RateLimitConfig::default());
        let router_id = RouterId::Named(CompactString::new("managed"));
        let router = RouterConfig {
            rate_limit: Some(RateLimitConfig::default()),
            ..RouterConfig::default()
        };
        config.routers = RouterConfigs::new(
            [(router_id.clone(), router)].into_iter().collect(),
        );

        apply_dev_rate_limit_overrides(&mut config);

        assert!(config.global.rate_limit.is_none());
        assert!(config.unified_api.rate_limit.is_none());
        assert!(config.routers[&router_id].rate_limit.is_none());
    }
}
