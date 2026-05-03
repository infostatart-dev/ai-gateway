use std::time::Duration;

use serde::{Deserialize, Serialize};

#[derive(
    Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq, Hash,
)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct DecisionEngineConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub shaper: TrafficShaperConfig,
    #[serde(default)]
    pub policy_store: PolicyStoreConfig,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_policy: Option<DecisionPolicyConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state_store: Option<StateStoreConfig>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, Hash)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct TrafficShaperConfig {
    #[serde(default = "default_global_limit")]
    pub global: usize,
    #[serde(default = "default_free_tier_limit")]
    pub free_tier: usize,
    #[serde(default = "default_paid_tier_limit")]
    pub paid_tier: usize,
    #[serde(default = "default_provider_limit")]
    pub provider: usize,
    #[serde(with = "humantime_serde", default = "default_acquire_timeout")]
    pub acquire_timeout: Duration,
}

impl Default for TrafficShaperConfig {
    fn default() -> Self {
        Self {
            global: default_global_limit(),
            free_tier: default_free_tier_limit(),
            paid_tier: default_paid_tier_limit(),
            provider: default_provider_limit(),
            acquire_timeout: default_acquire_timeout(),
        }
    }
}

fn default_global_limit() -> usize {
    200
}
fn default_free_tier_limit() -> usize {
    50
}
fn default_paid_tier_limit() -> usize {
    150
}
fn default_provider_limit() -> usize {
    200
}
fn default_acquire_timeout() -> Duration {
    Duration::from_secs(2)
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, Hash)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct PolicyStoreConfig {
    #[serde(default = "default_policy_cache_capacity")]
    pub cache_capacity: u64,
    #[serde(with = "humantime_serde", default = "default_policy_cache_ttl")]
    pub cache_ttl: Duration,
}

impl Default for PolicyStoreConfig {
    fn default() -> Self {
        Self {
            cache_capacity: default_policy_cache_capacity(),
            cache_ttl: default_policy_cache_ttl(),
        }
    }
}

fn default_policy_cache_capacity() -> u64 {
    1000
}
fn default_policy_cache_ttl() -> Duration {
    Duration::from_mins(5)
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, Hash)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct DecisionPolicyConfig {
    #[serde(default)]
    pub tier: DecisionTier,
    #[serde(default = "default_max_output_tokens")]
    pub max_output_tokens: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub budget_namespace: Option<String>,
    #[serde(default)]
    pub allow_hedging: bool,
    #[serde(default)]
    pub allow_delay: bool,
}

impl Default for DecisionPolicyConfig {
    fn default() -> Self {
        Self {
            tier: DecisionTier::default(),
            max_output_tokens: default_max_output_tokens(),
            budget_namespace: None,
            allow_hedging: false,
            allow_delay: false,
        }
    }
}

#[derive(
    Debug, Clone, Copy, Default, Deserialize, Serialize, PartialEq, Eq, Hash,
)]
#[serde(rename_all = "kebab-case")]
pub enum DecisionTier {
    #[default]
    Free,
    Paid,
}

fn default_max_output_tokens() -> u32 {
    4_000
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, Hash)]
#[serde(deny_unknown_fields, rename_all = "kebab-case", tag = "type")]
pub enum StateStoreConfig {
    Memory,
    Redis(crate::config::redis::RedisConfig),
}
