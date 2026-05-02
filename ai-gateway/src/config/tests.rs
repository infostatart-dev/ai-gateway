use std::time::Duration;

use regex::Regex;

use crate::config::{
    Config, ROUTER_ID_REGEX, deployment_target::DeploymentTarget,
};

#[test]
fn router_id_regex_is_valid() {
    assert!(Regex::new(ROUTER_ID_REGEX).is_ok());
}

#[test]
fn default_config_is_serializable() {
    let _ = serde_json::to_string(&Config::default()).unwrap();
}

#[test]
fn deployment_target_round_trip() {
    let config = DeploymentTarget::Sidecar;
    let ser = serde_json::to_string(&config).unwrap();
    assert_eq!(config, serde_json::from_str(&ser).unwrap());

    let cloud = DeploymentTarget::Cloud {
        db_poll_interval: Duration::from_secs(60),
        listener_reconnect_interval: Duration::from_secs(300),
    };
    let ser = serde_json::to_string(&cloud).unwrap();
    assert_eq!(cloud, serde_json::from_str(&ser).unwrap());
}

#[test]
fn router_id_regex_positive_cases() {
    let r = Regex::new(ROUTER_ID_REGEX).unwrap();
    for id in ["a", "Z", "abc", "A1B2", "a-1", "a_b", "0123456789"] {
        assert!(r.is_match(id));
    }
}
