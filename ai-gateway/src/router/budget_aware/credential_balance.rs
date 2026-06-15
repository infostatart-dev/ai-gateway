//! Round-robin among upstream account keys for the same provider + model.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use indexmap::IndexMap;

use super::types::BudgetCandidate;
use crate::types::provider::InferenceProvider;

pub(super) type AccountPoolKey = (InferenceProvider, String);

#[derive(Debug, Default)]
pub(super) struct CredentialRoundRobin {
    inner: Mutex<HashMap<AccountPoolKey, usize>>,
}

impl CredentialRoundRobin {
    #[must_use]
    pub fn new_shared() -> Arc<Self> {
        Arc::new(Self::default())
    }

    pub(super) fn balance(&self, ranked: Vec<BudgetCandidate>) -> Vec<BudgetCandidate> {
        let mut counters = self
            .inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        balance_credentials_among_accounts(ranked, &mut counters)
    }
}

fn account_pool_key(candidate: &BudgetCandidate) -> AccountPoolKey {
    (
        candidate.capability.provider.clone(),
        candidate.capability.model.to_string(),
    )
}

/// Groups ranked candidates by `(provider, model)`, keeps provider order, rotates
/// account keys inside each pool.
pub(super) fn balance_credentials_among_accounts(
    ranked: Vec<BudgetCandidate>,
    round_robin: &mut HashMap<AccountPoolKey, usize>,
) -> Vec<BudgetCandidate> {
    let mut pools: IndexMap<AccountPoolKey, Vec<BudgetCandidate>> =
        IndexMap::new();
    for candidate in ranked {
        pools
            .entry(account_pool_key(&candidate))
            .or_default()
            .push(candidate);
    }

    let mut balanced = Vec::with_capacity(
        pools.values().map(std::vec::Vec::len).sum(),
    );
    for (key, mut accounts) in pools {
        accounts.sort_by(|left, right| {
            left.credential_budget_rank
                .cmp(&right.credential_budget_rank)
                .then_with(|| {
                    left.credential_id.to_string().cmp(&right.credential_id.to_string())
                })
        });
        if accounts.len() <= 1 {
            balanced.extend(accounts);
            continue;
        }
        let offset = {
            let counter = round_robin.entry(key).or_insert(0);
            let offset = *counter % accounts.len();
            *counter += 1;
            offset
        };
        balanced.extend(
            accounts[offset..]
                .iter()
                .chain(accounts[..offset].iter())
                .cloned(),
        );
    }
    balanced
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::AccountPoolKey;
    use crate::types::provider::InferenceProvider;
    use indexmap::IndexMap;

    #[derive(Clone)]
    struct PoolEntry {
        credential_id: String,
        provider: String,
        model: String,
        budget_rank: u16,
    }

    fn balance_entries(
        ranked: Vec<PoolEntry>,
        round_robin: &mut HashMap<AccountPoolKey, usize>,
    ) -> Vec<String> {
        let mut pools: IndexMap<AccountPoolKey, Vec<PoolEntry>> =
            IndexMap::new();
        for entry in ranked {
            let key = (
                InferenceProvider::Named(entry.provider.clone().into()),
                entry.model.clone(),
            );
            pools.entry(key).or_default().push(entry);
        }
        let mut ids = Vec::new();
        for (key, mut accounts) in pools {
            accounts.sort_by(|left, right| {
                left.budget_rank
                    .cmp(&right.budget_rank)
                    .then_with(|| left.credential_id.cmp(&right.credential_id))
            });
            if accounts.len() <= 1 {
                ids.extend(accounts.into_iter().map(|a| a.credential_id));
                continue;
            }
            let offset = {
                let counter = round_robin.entry(key).or_insert(0);
                let offset = *counter % accounts.len();
                *counter += 1;
                offset
            };
            ids.extend(
                accounts[offset..]
                    .iter()
                    .chain(accounts[..offset].iter())
                    .map(|a| a.credential_id.clone()),
            );
        }
        ids
    }

    #[test]
    fn groups_scattered_accounts_for_same_provider_model() {
        let ranked = vec![
            PoolEntry {
                credential_id: "groq-default".into(),
                provider: "groq".into(),
                model: "llama".into(),
                budget_rank: 0,
            },
            PoolEntry {
                credential_id: "gemini-free".into(),
                provider: "gemini".into(),
                model: "gemini-2.5-flash".into(),
                budget_rank: 0,
            },
            PoolEntry {
                credential_id: "anthropic-default".into(),
                provider: "anthropic".into(),
                model: "claude".into(),
                budget_rank: 0,
            },
            PoolEntry {
                credential_id: "gemini-default".into(),
                provider: "gemini".into(),
                model: "gemini-2.5-flash".into(),
                budget_rank: 10,
            },
        ];

        let mut rr = HashMap::new();
        assert_eq!(
            balance_entries(ranked, &mut rr),
            vec![
                "groq-default",
                "gemini-free",
                "gemini-default",
                "anthropic-default",
            ]
        );
    }

    #[test]
    fn alternates_first_account_across_requests() {
        let ranked = vec![
            PoolEntry {
                credential_id: "gemini-free".into(),
                provider: "gemini".into(),
                model: "gemini-2.5-flash".into(),
                budget_rank: 0,
            },
            PoolEntry {
                credential_id: "gemini-default".into(),
                provider: "gemini".into(),
                model: "gemini-2.5-flash".into(),
                budget_rank: 10,
            },
        ];
        let mut rr = HashMap::new();

        assert_eq!(
            balance_entries(ranked.clone(), &mut rr),
            vec!["gemini-free", "gemini-default"]
        );
        assert_eq!(
            balance_entries(ranked.clone(), &mut rr),
            vec!["gemini-default", "gemini-free"]
        );
        assert_eq!(
            balance_entries(ranked, &mut rr),
            vec!["gemini-free", "gemini-default"]
        );
    }

    #[test]
    fn openrouter_accounts_with_same_rank_alternate() {
        let ranked = vec![
            PoolEntry {
                credential_id: "openrouter-a".into(),
                provider: "openrouter".into(),
                model: "gpt-oss".into(),
                budget_rank: 0,
            },
            PoolEntry {
                credential_id: "openrouter-b".into(),
                provider: "openrouter".into(),
                model: "gpt-oss".into(),
                budget_rank: 0,
            },
        ];
        let mut rr = HashMap::new();

        assert_eq!(
            balance_entries(ranked.clone(), &mut rr),
            vec!["openrouter-a", "openrouter-b"]
        );
        assert_eq!(
            balance_entries(ranked, &mut rr),
            vec!["openrouter-b", "openrouter-a"]
        );
    }
}
