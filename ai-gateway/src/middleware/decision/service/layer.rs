use tower::Layer;

use crate::{
    app_state::AppState,
    config::{decision::TierCascade, router::RouterConfig},
    types::router::RouterId,
};

#[derive(Clone)]
pub struct DecisionEngineLayer {
    app_state: AppState,
    decision_enabled: bool,
    tier_cascade_override: Option<TierCascade>,
}

impl DecisionEngineLayer {
    #[must_use]
    pub fn new(
        app_state: AppState,
        _router_id: RouterId,
        router_config: &RouterConfig,
    ) -> Self {
        Self {
            app_state,
            decision_enabled: router_config.decision.enabled,
            tier_cascade_override: router_config.decision.tier_cascade,
        }
    }
}

#[derive(Clone)]
pub struct DecisionEngineService<S> {
    pub(super) inner: S,
    pub(super) app_state: AppState,
    pub(super) decision_enabled: bool,
    pub(super) tier_cascade_override: Option<TierCascade>,
}

impl<S> Layer<S> for DecisionEngineLayer {
    type Service = DecisionEngineService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        DecisionEngineService {
            inner,
            app_state: self.app_state.clone(),
            decision_enabled: self.decision_enabled,
            tier_cascade_override: self.tier_cascade_override,
        }
    }
}

#[cfg(test)]
mod tests {
    use compact_str::CompactString;

    use super::*;
    use crate::types::router::RouterId;

    #[tokio::test]
    async fn layer_uses_router_decision_flag() {
        let app_state = AppState::test_default().await;
        let mut router_config = RouterConfig::default();
        router_config.decision.enabled = true;

        let layer = DecisionEngineLayer::new(
            app_state,
            RouterId::Named(CompactString::new("decision-router")),
            &router_config,
        );

        assert!(layer.decision_enabled);
    }
}
