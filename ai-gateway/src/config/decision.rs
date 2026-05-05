use std::{str::FromStr, time::Duration};

use indexmap::{IndexMap, IndexSet};
use serde::{Deserialize, Serialize};

use crate::types::model_id::{ModelId, ModelIdWithoutVersion};

#[derive(
    Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq,
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
    /// Маппинг модель → тир. Если задан, capability-aware router
    /// фильтрует кандидатов по `policy.tier` (с cascade при exhausted).
    /// Если пуст — tier-фильтр выключен, поведение как раньше.
    #[serde(default)]
    pub model_tiers: ModelTiersConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, Hash)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct TrafficShaperConfig {
    #[serde(default = "default_global_limit")]
    pub global: usize,
    #[serde(default = "default_free_tier_limit")]
    pub free_tier: usize,
    #[serde(default = "default_freemium_tier_limit")]
    pub freemium_tier: usize,
    #[serde(default = "default_paid_tier_limit")]
    pub paid_tier: usize,
    #[serde(default = "default_provider_limit")]
    pub provider: usize,
    #[serde(with = "humantime_serde", default = "default_acquire_timeout")]
    pub acquire_timeout: Duration,
    #[serde(default)]
    pub cascade: TierCascade,
}

impl Default for TrafficShaperConfig {
    fn default() -> Self {
        Self {
            global: default_global_limit(),
            free_tier: default_free_tier_limit(),
            freemium_tier: default_freemium_tier_limit(),
            paid_tier: default_paid_tier_limit(),
            provider: default_provider_limit(),
            acquire_timeout: default_acquire_timeout(),
            cascade: TierCascade::default(),
        }
    }
}

fn default_global_limit() -> usize {
    200
}
fn default_free_tier_limit() -> usize {
    50
}
fn default_freemium_tier_limit() -> usize {
    100
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

/// Поведение шейпера при exhausted-слоте стартового тира:
/// - `OnlyTier` — без cascade, отдаём ошибку acquire timeout.
/// - `PaidDown` — пробуем тиры в порядке `paid → freemium → free`,
///   стартуя с того, что прописан в политике, и спускаясь дешевле.
/// - `FreeUp` — `free → freemium → paid`, поднимаемся к более дорогим.
#[derive(
    Debug, Clone, Copy, Default, Deserialize, Serialize, PartialEq, Eq, Hash,
)]
#[serde(rename_all = "kebab-case")]
pub enum TierCascade {
    #[default]
    OnlyTier,
    PaidDown,
    FreeUp,
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
    Freemium,
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

/// Маппинг тир → список моделей. Конфигурируется в YAML как
/// `model-tiers: { free: [...], freemium: [...], paid: [...] }`.
/// Сравнение моделей идёт по `ModelIdWithoutVersion` чтобы версионные
/// суффиксы (`-2024-01-01`, `:free`, и т.п.) не мешали матчингу.
#[derive(
    Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq,
)]
#[serde(transparent)]
pub struct ModelTiersConfig(pub IndexMap<DecisionTier, IndexSet<String>>);

impl ModelTiersConfig {
    /// Резолвит тир для конкретной `ModelId`. Сравнение по
    /// `ModelIdWithoutVersion`, поэтому `gpt-4o` в конфиге матчит
    /// и `gpt-4o-2024-08-06` в реальной модели.
    #[must_use]
    pub fn tier_of(&self, model: &ModelId) -> Option<DecisionTier> {
        let target = ModelIdWithoutVersion::from(model.clone());
        let provider_hint = model.inference_provider();
        for (tier, models) in &self.0 {
            for raw in models {
                // Сначала с явно прокинутым провайдером модели запроса.
                if let Some(provider) = provider_hint.as_ref() {
                    if let Ok(parsed) = ModelId::from_str_and_provider(
                        provider.clone(),
                        raw,
                    ) {
                        if ModelIdWithoutVersion::from(parsed) == target {
                            return Some(*tier);
                        }
                    }
                }
                // Иначе пробуем парсить как полный ModelId
                // (например, "openai/gpt-4o" уже содержит провайдера).
                if let Ok(parsed) = ModelId::from_str(raw) {
                    if ModelIdWithoutVersion::from(parsed) == target {
                        return Some(*tier);
                    }
                }
            }
        }
        None
    }

    /// Все объявленные тиры в порядке записи в YAML.
    pub fn tiers(&self) -> impl Iterator<Item = DecisionTier> + '_ {
        self.0.keys().copied()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}
