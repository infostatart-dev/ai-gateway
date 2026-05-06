use crate::{config::Config, error::init::InitError};

pub(super) fn validate_decision_config(
    config: &Config,
) -> Result<(), InitError> {
    if config.decision.default_policy.max_output_tokens == 0 {
        return Err(InitError::InvalidDecisionConfig(
            "default policy max output tokens must be greater than zero",
        ));
    }
    Ok(())
}
