use uuid::Uuid;

use super::KeyPolicy;
use crate::{
    config::decision::DecisionPolicyConfig,
    types::{org::OrgId, secret::Secret, user::UserId},
};

#[test]
fn default_policy_derives_authenticated_namespace() {
    let org_id = OrgId::new(Uuid::new_v4());
    let user_id = UserId::new(Uuid::new_v4());
    let auth = crate::types::extensions::AuthContext {
        api_key: Secret::from("key".to_string()),
        user_id,
        org_id,
    };

    let policy =
        KeyPolicy::from_config(&DecisionPolicyConfig::default(), Some(&auth));

    assert_eq!(
        policy.budget_namespace,
        format!("decision:{org_id}:{user_id}")
    );
}
