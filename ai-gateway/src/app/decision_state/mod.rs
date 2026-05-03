//! Decision engine components built at application startup.

mod store;
mod validate;

use std::sync::Arc;

use crate::{config::Config, error::init::InitError};

pub(crate) struct DecisionState {
    pub traffic_shaper:
        Arc<crate::middleware::decision::shaping::TrafficShaper>,
    pub state_store: Arc<dyn crate::middleware::decision::budget::StateStore>,
    pub policy_store: Arc<dyn crate::middleware::decision::policy::PolicyStore>,
}

pub(crate) fn build_decision_state(
    config: &Config,
) -> Result<DecisionState, InitError> {
    validate::validate_decision_config(config)?;

    Ok(DecisionState {
        traffic_shaper: Arc::new(
            crate::middleware::decision::shaping::TrafficShaper::new(
                config.decision.shaper.global,
                config.decision.shaper.free_tier,
                config.decision.shaper.paid_tier,
                config.decision.shaper.provider,
            ),
        ),
        state_store: store::build_decision_state_store(config)?,
        policy_store: Arc::new(
            crate::middleware::decision::policy::MemoryPolicyStore::new(
                config.decision.policy_store.cache_capacity,
                config.decision.policy_store.cache_ttl,
                config.decision.default_policy.clone(),
            ),
        ),
    })
}
