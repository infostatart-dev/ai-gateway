#[cfg(all(test, feature = "testing"))]
mod tests {
    use super::super::{InnerLayer, Layer};
    use crate::{
        app_state::AppState,
        config::{
            Config,
            rate_limit::{
                GcraConfig, LimitsConfig, RateLimitConfig, RateLimitStore,
            },
            router::RouterConfig,
        },
        tests::TestDefault,
        types::router::RouterId,
    };
    use compact_str::CompactString;
    use std::{num::NonZeroU32, time::Duration};

    async fn create_test_app_state(rl_config: RateLimitConfig) -> AppState {
        let mut config = Config::test_default();
        config.global.rate_limit = Some(rl_config);
        crate::app::App::new(config)
            .await
            .expect("failed to create app")
            .state
    }

    fn create_test_limits() -> LimitsConfig {
        LimitsConfig {
            per_api_key: GcraConfig {
                capacity: NonZeroU32::new(10).unwrap(),
                refill_frequency: Duration::from_secs(1),
            },
        }
    }

    #[tokio::test]
    async fn global_app_with_none_router() {
        let app_state = create_test_app_state(RateLimitConfig {
            store: None,
            limits: create_test_limits(),
        })
        .await;
        let result = Layer::per_router(
            &app_state,
            RouterId::Named(CompactString::new("my-router")),
            &RouterConfig::default(),
        )
        .await;
        assert!(result.is_ok());
        assert!(matches!(result.unwrap().inner, InnerLayer::None));
    }
}
