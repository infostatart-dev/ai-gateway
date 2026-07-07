use std::{collections::HashMap, fmt};

use compact_str::CompactString;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use crate::{
    config::{
        cost_class::{self, CostClass},
        providers::ProvidersConfig,
        secrets_file::SecretsFile,
    },
    types::{
        provider::{InferenceProvider, ProviderKey},
        secret::Secret,
    },
};

const CREDENTIALS_YAML: &str =
    include_str!("../../config/embedded/credentials.yaml");

#[must_use]
pub fn requires_keyless_secret_opt_in(provider: &InferenceProvider) -> bool {
    matches!(provider, InferenceProvider::Named(name) if name == "vllm")
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ProviderCredentialId(pub CompactString);

impl ProviderCredentialId {
    #[must_use]
    pub fn new(id: impl Into<CompactString>) -> Self {
        Self(id.into())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl AsRef<str> for ProviderCredentialId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ProviderCredentialId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderCredential {
    pub id: ProviderCredentialId,
    pub provider: InferenceProvider,
    pub tier: String,
    pub cost_class: CostClass,
    pub key: ProviderKey,
    pub budget_rank: u16,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CredentialRegistry {
    credentials: Vec<ProviderCredential>,
    by_provider: IndexMap<InferenceProvider, Vec<usize>>,
    default_by_provider: HashMap<InferenceProvider, ProviderCredentialId>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "kebab-case")]
struct CredentialCatalog {
    credentials: IndexMap<String, CredentialSpec>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "kebab-case")]
struct CredentialSpec {
    provider: InferenceProvider,
    tier: String,
    #[serde(default)]
    cost_class: Option<CostClass>,
    #[serde(default)]
    budget_rank: u16,
}

impl CredentialSpec {
    fn resolved_cost_class(&self) -> CostClass {
        cost_class::derive_cost_class(
            self.cost_class,
            &self.provider,
            &self.tier,
        )
    }
}

impl CredentialRegistry {
    #[must_use]
    pub fn build(
        providers_config: &ProvidersConfig,
        secrets: &mut SecretsFile,
    ) -> Self {
        let catalog: CredentialCatalog = serde_yml::from_str(CREDENTIALS_YAML)
            .expect("embedded credentials.yaml must parse");
        let mut registry = Self::default();

        for (id, spec) in catalog.credentials {
            let cost_class = spec.resolved_cost_class();
            let Some(key) = secrets.resolve_provider_key(&id, &spec.provider)
            else {
                continue;
            };
            if !providers_config.contains_key(&spec.provider) {
                continue;
            }
            registry.push(ProviderCredential {
                id: ProviderCredentialId::new(id),
                provider: spec.provider,
                tier: spec.tier,
                cost_class,
                key,
                budget_rank: spec.budget_rank,
            });
        }

        registry.push_bedrock_from_integrations(providers_config, secrets);
        registry.rebuild_indexes();
        registry
    }

    fn push_bedrock_from_integrations(
        &mut self,
        providers_config: &ProvidersConfig,
        secrets: &SecretsFile,
    ) {
        let bedrock = InferenceProvider::Bedrock;
        if !providers_config.contains_key(&bedrock) {
            return;
        }
        if self.has_for(&bedrock) {
            return;
        }
        let Some(aws) = secrets.integrations.aws.as_ref() else {
            return;
        };
        if aws.access_key.is_empty() || aws.secret_key.is_empty() {
            return;
        }
        self.push(ProviderCredential {
            id: ProviderCredentialId::new("bedrock-default"),
            provider: bedrock.clone(),
            tier: "default".into(),
            cost_class: cost_class::derive_cost_class(
                None, &bedrock, "default",
            ),
            key: ProviderKey::AwsCredentials {
                access_key: Secret::from(aws.access_key.clone()),
                secret_key: Secret::from(aws.secret_key.clone()),
            },
            budget_rank: 0,
        });
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.credentials.is_empty()
    }

    #[must_use]
    pub fn all(&self) -> &[ProviderCredential] {
        &self.credentials
    }

    #[must_use]
    pub fn has_for(&self, provider: &InferenceProvider) -> bool {
        self.credentials
            .iter()
            .any(|credential| &credential.provider == provider)
    }

    pub fn for_provider(
        &self,
        provider: &InferenceProvider,
    ) -> impl Iterator<Item = &ProviderCredential> {
        self.by_provider
            .get(provider)
            .into_iter()
            .flat_map(|indices| indices.iter().map(|i| &self.credentials[*i]))
    }

    #[must_use]
    pub fn get(
        &self,
        id: &ProviderCredentialId,
    ) -> Option<&ProviderCredential> {
        self.credentials.iter().find(|c| &c.id == id)
    }

    #[must_use]
    pub fn default_for(
        &self,
        provider: &InferenceProvider,
    ) -> Option<&ProviderCredential> {
        let id = self.default_by_provider.get(provider)?;
        self.credentials.iter().find(|c| &c.id == id)
    }

    #[must_use]
    pub fn default_key(
        &self,
        provider: &InferenceProvider,
    ) -> Option<ProviderKey> {
        self.default_for(provider).map(|c| c.key.clone())
    }

    fn push(&mut self, credential: ProviderCredential) {
        self.credentials.push(credential);
    }

    fn rebuild_indexes(&mut self) {
        self.by_provider.clear();
        self.default_by_provider.clear();

        let mut order: Vec<usize> = (0..self.credentials.len()).collect();
        order.sort_by_key(|i| {
            (
                self.credentials[*i].budget_rank,
                self.credentials[*i].id.0.clone(),
            )
        });

        for index in order {
            let credential = &self.credentials[index];
            self.by_provider
                .entry(credential.provider.clone())
                .or_default()
                .push(index);
            self.default_by_provider
                .entry(credential.provider.clone())
                .or_insert_with(|| credential.id.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;
    use crate::config::secrets_file::SecretsFile;

    fn providers() -> ProvidersConfig {
        ProvidersConfig::default()
    }

    fn write_secrets(dir: &Path, yaml: &str) -> SecretsFile {
        let path = dir.join("secrets.yaml");
        std::fs::write(&path, yaml).unwrap();
        SecretsFile::load(&path).unwrap()
    }

    #[test]
    fn catalog_parses_embedded_yaml() {
        let catalog: CredentialCatalog =
            serde_yml::from_str(CREDENTIALS_YAML).unwrap();
        assert!(catalog.credentials.contains_key("gemini-free"));
        assert!(catalog.credentials.contains_key("gemini-free-16"));
        assert!(catalog.credentials.contains_key("vllm-anonymous"));
        assert!(catalog.credentials.contains_key("deepseek-web-2"));
        assert!(catalog.credentials.contains_key("openrouter-default"));
        assert!(catalog.credentials.contains_key("longcat-default"));
        assert!(catalog.credentials.contains_key("cohere-default"));
        let groq = catalog.credentials.get("groq-default").unwrap();
        assert_eq!(groq.tier, "free");
        assert_eq!(groq.cost_class, Some(CostClass::Free));
    }

    #[test]
    fn registry_loads_slot_from_secrets_file() {
        let dir = std::env::temp_dir().join("ai-gw-cred-openrouter");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut secrets = write_secrets(
            &dir,
            "credentials:\n  openrouter-default:\n    api-key: sk-or-test\n",
        );
        let registry = CredentialRegistry::build(&providers(), &mut secrets);
        assert!(registry.has_for(&InferenceProvider::OpenRouter));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn legacy_env_is_ignored_without_secrets_file_entry() {
        unsafe {
            std::env::set_var(
                "AI_GATEWAY_CREDENTIAL_OPENROUTER_DEFAULT",
                "legacy",
            );
        }
        let mut secrets = SecretsFile::default();
        let registry = CredentialRegistry::build(&providers(), &mut secrets);
        assert!(!registry.has_for(&InferenceProvider::OpenRouter));
        unsafe {
            std::env::remove_var("AI_GATEWAY_CREDENTIAL_OPENROUTER_DEFAULT");
        }
    }

    #[test]
    fn registry_loads_multiple_gemini_free_slots() {
        let dir = std::env::temp_dir().join("ai-gw-cred-gemini");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut secrets = write_secrets(
            &dir,
            r"
credentials:
  gemini-free:
    api-key: free-1
  gemini-free-2:
    api-key: free-2
",
        );
        let registry = CredentialRegistry::build(&providers(), &mut secrets);
        let gemini: Vec<_> = registry
            .for_provider(&InferenceProvider::GoogleGemini)
            .collect();
        assert_eq!(gemini.len(), 2);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn registry_loads_vllm_only_with_keyless_secret_opt_in() {
        let mut empty = SecretsFile::default();
        let registry = CredentialRegistry::build(&providers(), &mut empty);
        let vllm = InferenceProvider::Named("vllm".into());
        assert!(requires_keyless_secret_opt_in(&vllm));
        assert!(!registry.has_for(&vllm));

        let dir = std::env::temp_dir().join("ai-gw-cred-vllm");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut secrets = write_secrets(
            &dir,
            "credentials:\n  vllm-anonymous:\n    keyless: true\n",
        );
        let registry = CredentialRegistry::build(&providers(), &mut secrets);
        let credential = registry.default_for(&vllm).expect("vllm credential");
        assert_eq!(credential.id.as_str(), "vllm-anonymous");
        assert_eq!(credential.key, ProviderKey::NotRequired);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn registry_loads_bedrock_from_integrations_aws() {
        let dir = std::env::temp_dir().join("ai-gw-cred-bedrock");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut secrets = write_secrets(
            &dir,
            r"
integrations:
  aws:
    access-key: AKIA
    secret-key: secret
    region: eu-central-1
",
        );
        let registry = CredentialRegistry::build(&providers(), &mut secrets);
        assert!(registry.has_for(&InferenceProvider::Bedrock));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn credential_id_display() {
        let id = ProviderCredentialId::new("gemini-free");
        assert_eq!(id.to_string(), "gemini-free");
    }
}
