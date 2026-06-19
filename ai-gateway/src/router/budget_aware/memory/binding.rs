use crate::config::credentials::ProviderCredentialId;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RouteBinding {
    pub credential_id: ProviderCredentialId,
    pub model: String,
}
