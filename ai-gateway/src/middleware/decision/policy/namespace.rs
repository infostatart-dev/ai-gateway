use crate::types::extensions::AuthContext;

pub(super) fn budget_namespace(auth: Option<&AuthContext>) -> String {
    auth.map_or_else(
        || "decision:anonymous".to_string(),
        |auth| format!("decision:{}:{}", auth.org_id, auth.user_id),
    )
}
