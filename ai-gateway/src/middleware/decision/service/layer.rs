use std::sync::Arc;

use tower::Layer;

use crate::{
    app_state::AppState,
    config::{decision::TierCascade, router::RouterConfig},
    types::router::RouterId,
};

#[derive(Clone)]
pub struct DecisionEngineLayer {
    app_state: AppState,
    tier_cascade_override: Option<TierCascade>,
}

impl DecisionEngineLayer {
    #[must_use]
    pub fn new(
        app_state: AppState,
        _router_id: RouterId,
        router_config: Arc<RouterConfig>,
    ) -> Self {
        Self {
            app_state,
            tier_cascade_override: router_config.decision_tier_cascade,
        }
    }
}

#[derive(Clone)]
pub struct DecisionEngineService<S> {
    pub(super) inner: S,
    pub(super) app_state: AppState,
    pub(super) tier_cascade_override: Option<TierCascade>,
}

impl<S> Layer<S> for DecisionEngineLayer {
    type Service = DecisionEngineService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        DecisionEngineService {
            inner,
            app_state: self.app_state.clone(),
            tier_cascade_override: self.tier_cascade_override,
        }
    }
}
