use std::collections::HashMap;

use ai_gateway::{
    config::secrets_file::SecretsFile,
    types::provider::{InferenceProvider, ProviderKey},
};
use indexmap::IndexMap;
use serde::Deserialize;

const CREDENTIALS_YAML: &str =
    include_str!("../../../ai-gateway/config/embedded/credentials.yaml");

#[derive(Debug, Clone)]
#[allow(dead_code)]
struct TierEntry {
    provider: InferenceProvider,
    tier: String,
}

#[derive(Debug, Default, Clone)]
pub struct CredentialTierMap {
    by_api_key: HashMap<String, TierEntry>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
struct CredentialCatalog {
    credentials: IndexMap<String, CredentialSpec>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
struct CredentialSpec {
    provider: InferenceProvider,
    tier: String,
}

impl CredentialTierMap {
    #[must_use]
    pub fn load() -> Self {
        let catalog: CredentialCatalog = serde_yml::from_str(CREDENTIALS_YAML)
            .expect("embedded credentials.yaml must parse");
        let mut secrets = SecretsFile::load_discovered();
        let mut by_api_key = HashMap::new();
        for (id, spec) in catalog.credentials {
            let Some(key) = secrets.resolve_provider_key(&id, &spec.provider)
            else {
                continue;
            };
            let Some(api_key) = api_key_string(&key) else {
                continue;
            };
            by_api_key.insert(
                api_key,
                TierEntry {
                    provider: spec.provider,
                    tier: spec.tier,
                },
            );
        }
        Self { by_api_key }
    }

    #[must_use]
    pub fn tier_for(
        &self,
        provider: &InferenceProvider,
        bearer: &str,
    ) -> Option<String> {
        self.by_api_key
            .get(bearer)
            .filter(|entry| &entry.provider == provider)
            .map(|entry| entry.tier.clone())
    }
}

fn api_key_string(key: &ProviderKey) -> Option<String> {
    match key {
        ProviderKey::Secret(secret) => Some(secret.expose().clone()),
        _ => None,
    }
}
