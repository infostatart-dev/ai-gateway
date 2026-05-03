use super::{namespace, tier::Tier};
use crate::{
    config::decision::DecisionPolicyConfig, types::extensions::AuthContext,
};

#[derive(Debug, Clone)]
pub struct KeyPolicy {
    pub tier: Tier,
    pub budget_namespace: String,
    pub max_output_tokens: u32,
    pub allow_hedging: bool,
    pub allow_delay: bool,
}

impl KeyPolicy {
    #[must_use]
    pub fn from_config(
        config: &DecisionPolicyConfig,
        auth: Option<&AuthContext>,
    ) -> Self {
        let budget_namespace = config
            .budget_namespace
            .clone()
            .unwrap_or_else(|| namespace::budget_namespace(auth));
        Self {
            tier: config.tier.into(),
            budget_namespace,
            max_output_tokens: config.max_output_tokens,
            allow_hedging: config.allow_hedging,
            allow_delay: config.allow_delay,
        }
    }
}
