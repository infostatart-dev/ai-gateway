use std::sync::Arc;

use tower::Layer;

use crate::{
    app_state::AppState, config::router::RouterConfig, types::router::RouterId,
};

#[derive(Clone)]
pub struct DecisionEngineLayer {
    app_state: AppState,
}

impl DecisionEngineLayer {
    #[must_use]
    pub fn new(
        app_state: AppState,
        _router_id: RouterId,
        _router_config: Arc<RouterConfig>,
    ) -> Self {
        Self { app_state }
    }
}

#[derive(Clone)]
pub struct DecisionEngineService<S> {
    pub(super) inner: S,
    pub(super) app_state: AppState,
}

impl<S> Layer<S> for DecisionEngineLayer {
    type Service = DecisionEngineService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        DecisionEngineService {
            inner,
            app_state: self.app_state.clone(),
        }
    }
}
