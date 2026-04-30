use crate::{
    config::{Config, ROUTER_ID_REGEX},
    error::init::InitError,
};
use regex::Regex;

impl Config {
    pub fn validate(&self) -> Result<(), InitError> {
        let regex =
            Regex::new(ROUTER_ID_REGEX).expect("always valid if tests pass");
        for (router_id, router_config) in self.routers.as_ref() {
            router_config.validate()?;
            if !regex.is_match(router_id.as_ref()) {
                return Err(InitError::InvalidRouterId(router_id.to_string()));
            }
        }
        Ok(())
    }
}
