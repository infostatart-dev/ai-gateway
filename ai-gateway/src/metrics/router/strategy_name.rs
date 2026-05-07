use crate::config::balance::BalanceConfigInner;

#[must_use]
pub fn strategy_label(config: &BalanceConfigInner) -> &'static str {
    match config {
        BalanceConfigInner::ProviderWeighted { .. } => "provider-weighted",
        BalanceConfigInner::BalancedLatency { .. } => "balanced-latency",
        BalanceConfigInner::ProviderFailover { .. } => "provider-failover",
        BalanceConfigInner::CapabilityAware { .. } => "capability-aware",
        BalanceConfigInner::BudgetAware { .. } => "budget-aware",
        BalanceConfigInner::BudgetAwareCapabilityAfter { .. } => {
            "budget-aware-capability-after"
        }
        BalanceConfigInner::ModelWeighted { .. } => "model-weighted",
        BalanceConfigInner::ModelLatency { .. } => "model-latency",
    }
}

#[cfg(test)]
mod tests {
    use nonempty_collections::nes;
    use rust_decimal::Decimal;

    use super::*;
    use crate::{
        config::balance::WeightedProvider, types::provider::InferenceProvider,
    };

    #[test]
    fn labels_match_kebab_case() {
        let cfg = BalanceConfigInner::ProviderWeighted {
            providers: nes![WeightedProvider {
                provider: InferenceProvider::OpenAI,
                weight: Decimal::ONE,
            }],
        };
        assert_eq!(strategy_label(&cfg), "provider-weighted");
    }
}
