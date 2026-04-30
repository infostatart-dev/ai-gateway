use super::*;

#[test]
fn test_deserialize_features_field_only() {
    let yaml = r#"
api-key: "sk-test-key"
base-url: "https://example.com"
websocket-url: "wss://example.com/ws"
features: "all"
"#;
    let config: HeliconeConfig = serde_yml::from_str(yaml).unwrap();
    assert_eq!(config.features, HeliconeFeatures::All);
}

#[test]
fn test_deserialize_all_flags_true() {
    let yaml = r#"
api-key: "sk-test-key"
authentication: true
observability: true
__prompts: true
"#;
    let config: HeliconeConfig = serde_yml::from_str(yaml).unwrap();
    assert_eq!(config.features, HeliconeFeatures::All);
}

#[test]
fn test_deserialize_auth_true_others_false() {
    let yaml = r#"
api-key: "sk-test-key"
authentication: true
"#;
    let config: HeliconeConfig = serde_yml::from_str(yaml).unwrap();
    assert_eq!(config.features, HeliconeFeatures::Auth);
}

#[test]
fn test_deserialize_observability_true_only() {
    let yaml = r#"
api-key: "sk-test-key"
observability: true
"#;
    let config: HeliconeConfig = serde_yml::from_str(yaml).unwrap();
    assert_eq!(config.features, HeliconeFeatures::Observability);
}

#[test]
fn test_deserialize_prompts_true_only() {
    let yaml = r#"
api-key: "sk-test-key"
__prompts: true
"#;
    let config: HeliconeConfig = serde_yml::from_str(yaml).unwrap();
    assert_eq!(config.features, HeliconeFeatures::Prompts);
}

#[test]
fn test_deserialize_features_takes_precedence() {
    let yaml = r#"
api-key: "sk-test-key"
features: "auth"
authentication: true
observability: true
"#;
    let config: HeliconeConfig = serde_yml::from_str(yaml).unwrap();
    assert_eq!(config.features, HeliconeFeatures::Auth);
}

#[test]
fn test_helper_methods() {
    let auth_config = HeliconeConfig {
        features: HeliconeFeatures::Auth,
        ..Default::default()
    };
    assert!(auth_config.is_auth_enabled());
    assert!(!auth_config.is_auth_disabled());
    assert!(!auth_config.is_observability_enabled());

    let all_config = HeliconeConfig {
        features: HeliconeFeatures::All,
        ..Default::default()
    };
    assert!(all_config.is_auth_enabled());
    assert!(all_config.is_observability_enabled());

    let none_config = HeliconeConfig {
        features: HeliconeFeatures::None,
        ..Default::default()
    };
    assert!(!none_config.is_auth_enabled());
    assert!(none_config.is_auth_disabled());
}
