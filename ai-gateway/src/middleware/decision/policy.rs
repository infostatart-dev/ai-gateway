use std::fmt;

use moka::future::Cache;

use crate::{
    config::decision::{DecisionPolicyConfig, DecisionTier},
    types::extensions::AuthContext,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Tier {
    Free,
    Paid,
}

#[derive(Debug, Clone)]
pub struct KeyPolicy {
    pub tier: Tier,
    pub budget_namespace: String,
    pub max_output_tokens: u32,
    pub allow_hedging: bool,
    pub allow_delay: bool,
}

impl KeyPolicy {
    #[must_use]
    pub fn from_config(
        config: &DecisionPolicyConfig,
        auth: Option<&AuthContext>,
    ) -> Self {
        let budget_namespace = config
            .budget_namespace
            .clone()
            .unwrap_or_else(|| budget_namespace(auth));
        Self {
            tier: config.tier.into(),
            budget_namespace,
            max_output_tokens: config.max_output_tokens,
            allow_hedging: config.allow_hedging,
            allow_delay: config.allow_delay,
        }
    }
}

impl From<DecisionTier> for Tier {
    fn from(value: DecisionTier) -> Self {
        match value {
            DecisionTier::Free => Self::Free,
            DecisionTier::Paid => Self::Paid,
        }
    }
}

#[async_trait::async_trait]
pub trait PolicyStore: Send + Sync + std::fmt::Debug + 'static {
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

fn budget_namespace(auth: Option<&AuthContext>) -> String {
    auth.map_or_else(
        || "decision:anonymous".to_string(),
        |auth| format!("decision:{}:{}", auth.org_id, auth.user_id),
    )
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use super::*;
    use crate::types::{org::OrgId, secret::Secret, user::UserId};

    #[test]
    fn default_policy_derives_authenticated_namespace() {
        let org_id = OrgId::new(Uuid::new_v4());
        let user_id = UserId::new(Uuid::new_v4());
        let auth = AuthContext {
            api_key: Secret::from("key".to_string()),
            user_id,
            org_id,
        };

        let policy = KeyPolicy::from_config(
            &DecisionPolicyConfig::default(),
            Some(&auth),
        );

        assert_eq!(
            policy.budget_namespace,
            format!("decision:{org_id}:{user_id}")
        );
    }
}
