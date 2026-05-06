use crate::{
    config::balance::BalanceConfigInner,
    router::budget_aware::default_provider_budget_rank,
    types::provider::InferenceProvider,
};

pub fn providers_for_display(
    balance: &BalanceConfigInner,
) -> (&'static str, Vec<InferenceProvider>) {
    match balance {
        BalanceConfigInner::BudgetAwareCapabilityAfter {
            provider_priorities,
            ..
        } => {
            let mut providers: Vec<_> =
                balance.providers().into_iter().collect();
            providers.sort_by(|left, right| {
                provider_priorities
                    .get(left)
                    .copied()
                    .unwrap_or_else(|| default_provider_budget_rank(left))
                    .cmp(
                        &provider_priorities
                            .get(right)
                            .copied()
                            .unwrap_or_else(|| {
                                default_provider_budget_rank(right)
                            }),
                    )
                    .then_with(|| left.to_string().cmp(&right.to_string()))
            });
            ("Providers-by-budget", providers)
        }
        _ => ("Providers", balance.providers().into_iter().collect()),
    }
}
