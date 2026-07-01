use std::{path::PathBuf, time::Duration};

use indexmap::{IndexMap, IndexSet};
use serde::{Deserialize, Serialize};

use crate::config::redis::RedisConfig;

#[derive(Debug, Clone, Deserialize, Serialize, Eq, PartialEq, Hash)]
#[serde(default, deny_unknown_fields, rename_all = "kebab-case")]
pub struct ClientAccessConfig {
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file: Option<PathBuf>,
    #[serde(with = "humantime_serde", default = "default_reload_interval")]
    pub reload_interval: Duration,
    #[serde(
        default = "default_max_body_bytes",
        deserialize_with = "deserialize_byte_size"
    )]
    pub max_body_bytes: usize,
    #[serde(default)]
    pub quota_store: ClientAccessQuotaStoreConfig,
}

impl Default for ClientAccessConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            file: None,
            reload_interval: default_reload_interval(),
            max_body_bytes: default_max_body_bytes(),
            quota_store: ClientAccessQuotaStoreConfig::default(),
        }
    }
}

fn default_reload_interval() -> Duration {
    Duration::from_secs(1)
}

fn default_max_body_bytes() -> usize {
    4 * 1024 * 1024
}

#[derive(
    Debug, Clone, Default, Deserialize, Serialize, Eq, PartialEq, Hash,
)]
#[serde(deny_unknown_fields, rename_all = "kebab-case", tag = "type")]
pub enum ClientAccessQuotaStoreConfig {
    #[default]
    Memory,
    Redis(RedisConfig),
}

#[derive(Debug, Clone, Deserialize, Serialize, Eq, PartialEq)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct ClientAccessRegistryFile {
    pub version: u16,
    #[serde(default)]
    pub subjects: IndexMap<String, ClientAccessSubjectConfig>,
    pub plans: IndexMap<String, ClientAccessPlanConfig>,
    #[serde(default)]
    pub keys: IndexMap<String, ClientAccessKeyConfig>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Eq, PartialEq)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct ClientAccessSubjectConfig {
    pub org_id: crate::types::org::OrgId,
    pub user_id: crate::types::user::UserId,
}

#[derive(Debug, Clone, Deserialize, Serialize, Eq, PartialEq)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct ClientAccessPlanConfig {
    #[serde(default = "default_max_output_tokens")]
    pub max_output_tokens: u32,
    pub limits: ClientAccessLimitsConfig,
}

fn default_max_output_tokens() -> u32 {
    4_000
}

#[derive(Debug, Clone, Deserialize, Serialize, Eq, PartialEq)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct ClientAccessLimitsConfig {
    pub requests: ClientAccessWindowLimitsConfig,
    pub tokens: ClientAccessWindowLimitsConfig,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, Eq, PartialEq)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct ClientAccessWindowLimitsConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub per_minute: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub per_day: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub per_week: Option<u64>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Eq, PartialEq)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct ClientAccessKeyConfig {
    pub hash: String,
    pub subject: String,
    pub status: ClientAccessKeyStatus,
    pub plan: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<chrono::DateTime<chrono::Utc>>,
    pub scopes: IndexSet<String>,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, Eq, PartialEq, Hash)]
#[serde(rename_all = "kebab-case")]
pub enum ClientAccessKeyStatus {
    Active,
    Suspended,
}

fn deserialize_byte_size<'de, D>(deserializer: D) -> Result<usize, D::Error>
where
    D: serde::Deserializer<'de>,
{
    struct Visitor;

    impl serde::de::Visitor<'_> for Visitor {
        type Value = usize;

        fn expecting(
            &self,
            formatter: &mut std::fmt::Formatter<'_>,
        ) -> std::fmt::Result {
            formatter.write_str("a byte count integer or string like 4MiB")
        }

        fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            usize::try_from(value)
                .map_err(|_| E::custom("byte count exceeds usize"))
        }

        fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            parse_byte_size(value).map_err(E::custom)
        }
    }

    deserializer.deserialize_any(Visitor)
}

fn parse_byte_size(value: &str) -> Result<usize, String> {
    let trimmed = value.trim();
    let split_at = trimmed
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(trimmed.len());
    let (digits, suffix) = trimmed.split_at(split_at);
    if digits.is_empty() {
        return Err("byte size must start with a number".to_string());
    }
    let base = digits
        .parse::<usize>()
        .map_err(|_| "invalid byte size number".to_string())?;
    let multiplier = match suffix.trim().to_ascii_lowercase().as_str() {
        "" | "b" => 1,
        "k" | "kb" | "kib" => 1024,
        "m" | "mb" | "mib" => 1024 * 1024,
        "g" | "gb" | "gib" => 1024 * 1024 * 1024,
        other => return Err(format!("unsupported byte size suffix `{other}`")),
    };
    base.checked_mul(multiplier)
        .ok_or_else(|| "byte size overflows usize".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_defaults_disabled() {
        let cfg = ClientAccessConfig::default();
        assert!(!cfg.enabled);
        assert_eq!(cfg.max_body_bytes, 4 * 1024 * 1024);
        assert!(matches!(
            cfg.quota_store,
            ClientAccessQuotaStoreConfig::Memory
        ));
    }

    #[test]
    fn parses_human_body_size() {
        let cfg: ClientAccessConfig = serde_yml::from_str(
            r#"
enabled: true
file: ./client-access.yaml
max-body-bytes: 4MiB
"#,
        )
        .unwrap();
        assert_eq!(cfg.max_body_bytes, 4 * 1024 * 1024);
    }
}
