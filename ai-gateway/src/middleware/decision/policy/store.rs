use std::fmt;

use moka::future::Cache;

use super::key_policy::KeyPolicy;
use crate::{
    config::decision::DecisionPolicyConfig, types::extensions::AuthContext,
};

#[async_trait::async_trait]
pub trait PolicyStore: Send + Sync + fmt::Debug + 'static {
    async fn get_policy(&self, auth: Option<&AuthContext>)
    -> Option<KeyPolicy>;
}

pub struct MemoryPolicyStore {
    cache: Cache<String, KeyPolicy>,
    default_policy: Option<DecisionPolicyConfig>,
}

impl fmt::Debug for MemoryPolicyStore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MemoryPolicyStore").finish()
    }
}

impl MemoryPolicyStore {
    #[must_use]
    pub fn new(
        capacity: u64,
        ttl: std::time::Duration,
        default_policy: Option<DecisionPolicyConfig>,
    ) -> Self {
        Self {
            cache: Cache::builder()
                .max_capacity(capacity)
                .time_to_live(ttl)
                .build(),
            default_policy,
        }
    }

    pub async fn insert(&self, api_key: String, policy: KeyPolicy) {
        self.cache.insert(api_key, policy).await;
    }
}

#[async_trait::async_trait]
impl PolicyStore for MemoryPolicyStore {
    async fn get_policy(
        &self,
        auth: Option<&AuthContext>,
    ) -> Option<KeyPolicy> {
        if let Some(auth) = auth
            && let Some(policy) = self.cache.get(auth.api_key.expose()).await
        {
            return Some(policy);
        }

        self.default_policy
            .as_ref()
            .map(|policy| KeyPolicy::from_config(policy, auth))
    }
}
