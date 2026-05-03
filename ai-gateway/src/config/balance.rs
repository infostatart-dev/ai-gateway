use std::{collections::HashMap, time::Duration};

use derive_more::{AsRef, From};
use indexmap::{IndexMap, IndexSet};
use nonempty_collections::{NESet, nes};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::{
    endpoints::EndpointType,
    types::{model_id::ModelId, provider::InferenceProvider},
};

/// A registry of balance configs for each endpoint type,
/// since a separate load balancer is used for each endpoint type.
#[derive(Debug, Clone, Deserialize, Serialize, Eq, PartialEq, AsRef, From)]
pub struct BalanceConfig(pub HashMap<EndpointType, BalanceConfigInner>);

impl Default for BalanceConfig {
    fn default() -> Self {
        Self(HashMap::from([(
            EndpointType::Chat,
            BalanceConfigInner::BalancedLatency {
                providers: nes![
                    InferenceProvider::OpenAI,
                    InferenceProvider::Anthropic,
                    InferenceProvider::GoogleGemini,
                ],
            },
        )]))
    }
}

impl BalanceConfig {
    #[cfg(any(test, feature = "testing"))]
    #[must_use]
    pub fn openai_chat() -> Self {
        Self(HashMap::from([(
            EndpointType::Chat,
            BalanceConfigInner::ProviderWeighted {
                providers: nes![WeightedProvider {
                    provider: InferenceProvider::OpenAI,
                    weight: Decimal::from(1),
                }],
            },
        )]))
    }

    #[cfg(any(test, feature = "testing"))]
    #[must_use]
    pub fn anthropic_chat() -> Self {
        Self(HashMap::from([(
            EndpointType::Chat,
            BalanceConfigInner::ProviderWeighted {
                providers: nes![WeightedProvider {
                    provider: InferenceProvider::Anthropic,
                    weight: Decimal::from(1),
                }],
            },
        )]))
    }

    #[cfg(any(test, feature = "testing"))]
    #[must_use]
    pub fn google_gemini() -> Self {
        Self(HashMap::from([(
            EndpointType::Chat,
            BalanceConfigInner::ProviderWeighted {
                providers: nes![WeightedProvider {
                    provider: InferenceProvider::GoogleGemini,
                    weight: Decimal::from(1),
                }],
            },
        )]))
    }

    #[cfg(any(test, feature = "testing"))]
    #[must_use]
    pub fn ollama_chat() -> Self {
        Self(HashMap::from([(
            EndpointType::Chat,
            BalanceConfigInner::ProviderWeighted {
                providers: nes![WeightedProvider {
                    provider: InferenceProvider::Ollama,
                    weight: Decimal::from(1),
                }],
            },
        )]))
    }

    #[cfg(any(test, feature = "testing"))]
    #[must_use]
    pub fn bedrock() -> Self {
        Self(HashMap::from([(
            EndpointType::Chat,
            BalanceConfigInner::ProviderWeighted {
                providers: nes![WeightedProvider {
                    provider: InferenceProvider::Bedrock,
                    weight: Decimal::from(1),
                }],
            },
        )]))
    }

    #[cfg(any(test, feature = "testing"))]
    #[must_use]
    pub fn mistral() -> Self {
        Self(HashMap::from([(
            EndpointType::Chat,
            BalanceConfigInner::ProviderWeighted {
                providers: nes![WeightedProvider {
                    provider: InferenceProvider::Named("mistral".into()),
                    weight: Decimal::from(1),
                }],
            },
        )]))
    }

    #[must_use]
    pub fn providers(&self) -> IndexSet<InferenceProvider> {
        self.0
            .values()
            .flat_map(BalanceConfigInner::providers)
            .collect()
    }
}

/// Configurations which drive the strategy used for the
/// routing/load balancing done by the
/// [`RoutingStrategyService`](crate::router::strategy::RoutingStrategyService).
///
/// See the rustdocs there for more details.
#[derive(
    Debug, Clone, Deserialize, Serialize, Eq, PartialEq, strum::AsRefStr,
)]
#[strum(serialize_all = "kebab-case")]
#[serde(rename_all = "kebab-case", tag = "strategy")]
pub enum BalanceConfigInner {
    /// Distributes and load balances requests among a set of providers.
    #[serde(alias = "weighted")]
    ProviderWeighted { providers: NESet<WeightedProvider> },
    /// Distributes and load balances requests among a set of providers.
    /// This means there is an element of randomness in the selection of the
    /// provider, so generally requests will go to the provider with lowest
    /// latency, but not always.
    #[serde(alias = "latency")]
    BalancedLatency { providers: NESet<InferenceProvider> },
    /// Routes to the lowest-latency available provider and retries the same
    /// request against other providers when the selected provider is
    /// unavailable.
    #[serde(alias = "failover")]
    ProviderFailover { providers: NESet<InferenceProvider> },
    /// Selects provider/model based on request requirements and model
    /// capabilities.
    #[serde(alias = "capability-aware")]
    CapabilityAware { providers: NESet<InferenceProvider> },
    /// Selects capable provider/model candidates by budget priority first,
    /// waits briefly for cheap candidates to recover, and fails over to the
    /// next viable candidate on provider errors.
    #[serde(alias = "budget-aware")]
    BudgetAware {
        providers: NESet<InferenceProvider>,
        #[serde(default)]
        provider_priorities: IndexMap<InferenceProvider, u16>,
        #[serde(
            with = "humantime_serde",
            default = "default_budget_max_cooldown_wait"
        )]
        max_cooldown_wait: Duration,
    },
    /// Distributes and load balances requests among a set of (providers,model).
    ModelWeighted { models: NESet<WeightedModel> },
    /// Distributes and load balances requests among a set of (providers,model).
    ModelLatency { models: NESet<ModelId> },
}

impl BalanceConfigInner {
    #[must_use]
    pub fn providers(&self) -> IndexSet<InferenceProvider> {
        match self {
            Self::ProviderWeighted { providers } => {
                providers.iter().map(|t| t.provider.clone()).collect()
            }
            Self::BalancedLatency { providers }
            | Self::ProviderFailover { providers }
            | Self::CapabilityAware { providers }
            | Self::BudgetAware { providers, .. } => {
                providers.iter().cloned().collect()
            }
            Self::ModelWeighted { models } => models
                .iter()
                .filter_map(|model| {
                    if let Some(provider) = model.model.inference_provider() { Some(provider) } else {
                        tracing::warn!(model = ?model.model, "Model has no inference provider");
                        None
                    }
                })
                .collect(),
            Self::ModelLatency { models } => models
                .iter()
                .filter_map(|model| {
                    if let Some(provider) = model.inference_provider() { Some(provider) } else {
                        tracing::warn!(model = ?model, "Model has no inference provider");
                        None
                    }
                })
                .collect(),
        }
    }
}

pub(crate) const fn default_budget_max_cooldown_wait() -> Duration {
    Duration::from_secs(3)
}

#[derive(Debug, Clone, Deserialize, Serialize, Eq, Hash, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub struct WeightedProvider {
    pub provider: InferenceProvider,
    pub weight: Decimal,
}

#[derive(Debug, Clone, Deserialize, Serialize, Eq, Hash, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub struct WeightedModel {
    pub model: ModelId,
    pub weight: Decimal,
}
