use std::{collections::HashMap, fmt};

use compact_str::CompactString;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use crate::{
    config::providers::ProvidersConfig,
    types::provider::{InferenceProvider, ProviderKey},
};

const CREDENTIALS_YAML: &str =
    include_str!("../../config/embedded/credentials.yaml");

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ProviderCredentialId(pub CompactString);

impl ProviderCredentialId {
    #[must_use]
    pub fn new(id: impl Into<CompactString>) -> Self {
        Self(id.into())
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
    key_env: Option<String>,
    #[serde(default)]
    alt_key_envs: Vec<String>,
    #[serde(default)]
    budget_rank: u16,
}

impl CredentialRegistry {
    #[must_use]
    pub fn build(providers_config: &ProvidersConfig) -> Self {
        let catalog: CredentialCatalog = serde_yml::from_str(CREDENTIALS_YAML)
            .expect("embedded credentials.yaml must parse");
        let mut registry = Self::default();

        for (id, spec) in catalog.credentials {
            if crate::config::deepseek_web::is_deepseek_web(&spec.provider) {
                let Some(path) =
                    crate::config::deepseek_web::session_path_for_credential(
                        &id,
                    )
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
                    key: ProviderKey::Secret(
                        crate::types::secret::Secret::from(
                            path.display().to_string(),
                        ),
                    ),
                    budget_rank: spec.budget_rank,
                });
                continue;
            }

            if crate::config::perplexity_web::is_perplexity_web(&spec.provider)
            {
                let Some(path) =
                    crate::config::perplexity_web::session_path_for_credential(
                        &id,
                    )
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
                    key: ProviderKey::Secret(
                        crate::types::secret::Secret::from(
                            path.display().to_string(),
                        ),
                    ),
                    budget_rank: spec.budget_rank,
                });
                continue;
            }

            let extra_env = spec
                .key_env
                .iter()
                .chain(spec.alt_key_envs.iter())
                .cloned()
                .collect::<Vec<_>>();
            let Some(key) =
                crate::config::credential_env::resolve_credential_secret(
                    &id,
                    &spec.provider,
                    &extra_env,
                )
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
                key,
                budget_rank: spec.budget_rank,
            });
        }

        registry.fill_legacy_defaults(providers_config);
        registry.fill_session_credentials(providers_config);
        registry.rebuild_indexes();
        registry
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.credentials.is_empty()
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

    fn fill_session_credentials(&mut self, providers_config: &ProvidersConfig) {
        let chatgpt = InferenceProvider::Named("chatgpt-web".into());
        if providers_config.contains_key(&chatgpt)
            && !self.has_for(&chatgpt)
            && crate::config::chatgpt_web::session_file_available()
        {
            self.push(ProviderCredential {
                id: ProviderCredentialId::new("chatgpt-web-default"),
                provider: chatgpt,
                tier: "session".into(),
                key: ProviderKey::NotRequired,
                budget_rank: 0,
            });
        }

        let deepseek = InferenceProvider::Named("deepseek-web".into());
        if providers_config.contains_key(&deepseek)
            && !self.has_for(&deepseek)
            && crate::config::deepseek_web::session_file_available()
        {
            self.push(ProviderCredential {
                id: ProviderCredentialId::new("deepseek-web-default"),
                provider: deepseek,
                tier: "session".into(),
                key: ProviderKey::NotRequired,
                budget_rank: 0,
            });
        }
    }

    fn fill_legacy_defaults(&mut self, providers_config: &ProvidersConfig) {
        for (provider, _) in providers_config.iter() {
            if self.has_for(provider) {
                continue;
            }
            if provider.is_keyless() {
                continue;
            }
            let Some(key) = ProviderKey::from_env(provider) else {
                continue;
            };
            let id = ProviderCredentialId::new(format!("{provider}-default"));
            self.push(ProviderCredential {
                id,
                provider: provider.clone(),
                tier: "default".into(),
                key,
                budget_rank: 0,
            });
        }
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
    use std::str::FromStr;

    use super::*;

    fn providers() -> ProvidersConfig {
        ProvidersConfig::default()
    }

    #[test]
    fn catalog_parses_embedded_yaml() {
        let catalog: CredentialCatalog =
            serde_yml::from_str(CREDENTIALS_YAML).unwrap();
        assert!(catalog.credentials.contains_key("gemini-free"));
        assert!(catalog.credentials.contains_key("gemini-free-2"));
        assert!(catalog.credentials.contains_key("gemini-free-3"));
        assert!(catalog.credentials.contains_key("gemini-free-4"));
        assert!(catalog.credentials.contains_key("gemini-default"));
        assert!(catalog.credentials.contains_key("openrouter-default"));
        assert!(catalog.credentials.contains_key("deepseek-web-default"));
        let gemini_default = catalog.credentials.get("gemini-default").unwrap();
        assert_eq!(gemini_default.tier, "tier-3");
        assert_eq!(gemini_default.budget_rank, 10);
    }

    #[test]
    #[serial_test::serial(env)]
    fn registry_skips_deepseek_without_session_file() {
        unsafe {
            std::env::remove_var("DEEPSEEK_BROWSER_CLI");
            std::env::remove_var("AI_GATEWAY_CREDENTIAL_DEEPSEEK_WEB_DEFAULT");
        }
        let registry = CredentialRegistry::build(&providers());
        let deepseek = InferenceProvider::Named("deepseek-web".into());
        assert!(!registry.has_for(&deepseek));
    }

    #[test]
    #[serial_test::serial(env)]
    fn registry_loads_all_configured_free_gemini_siblings() {
        unsafe {
            std::env::set_var("AI_GATEWAY_CREDENTIAL_GEMINI_FREE", "free-1");
            std::env::set_var("AI_GATEWAY_CREDENTIAL_GEMINI_FREE_2", "free-2");
            std::env::set_var("AI_GATEWAY_CREDENTIAL_GEMINI_FREE_4", "free-4");
            std::env::remove_var("AI_GATEWAY_CREDENTIAL_GEMINI_FREE_3");
            std::env::remove_var("GEMINI_API_KEY");
        }
        let registry = CredentialRegistry::build(&providers());
        let gemini: Vec<_> = registry
            .for_provider(&InferenceProvider::GoogleGemini)
            .collect();
        assert_eq!(gemini.len(), 3);
        assert_eq!(gemini[0].id.0, "gemini-free");
        assert_eq!(gemini[1].id.0, "gemini-free-2");
        assert_eq!(gemini[2].id.0, "gemini-free-4");
        unsafe {
            std::env::remove_var("AI_GATEWAY_CREDENTIAL_GEMINI_FREE");
            std::env::remove_var("AI_GATEWAY_CREDENTIAL_GEMINI_FREE_2");
            std::env::remove_var("AI_GATEWAY_CREDENTIAL_GEMINI_FREE_4");
        }
    }

    #[test]
    #[serial_test::serial(env)]
    fn registry_loads_gemini_free_before_paid_from_env() {
        unsafe {
            std::env::set_var("GEMINI_FREE_TIER_APIKEY", "free-key");
            std::env::set_var("GEMINI_API_KEY", "paid-key");
        }
        let registry = CredentialRegistry::build(&providers());
        let gemini: Vec<_> = registry
            .for_provider(&InferenceProvider::GoogleGemini)
            .collect();
        assert_eq!(gemini.len(), 2);
        assert_eq!(gemini[0].id.0, "gemini-free");
        assert_eq!(gemini[0].tier, "free");
        assert_eq!(gemini[1].id.0, "gemini-default");
        assert_eq!(
            registry
                .default_for(&InferenceProvider::GoogleGemini)
                .unwrap()
                .id
                .0,
            "gemini-free"
        );
        unsafe {
            std::env::remove_var("GEMINI_FREE_TIER_APIKEY");
            std::env::remove_var("GEMINI_API_KEY");
        }
    }

    #[test]
    #[serial_test::serial(env)]
    fn registry_skips_missing_env_slots() {
        let clear = [
            "AI_GATEWAY_CREDENTIAL_GEMINI_FREE",
            "AI_GATEWAY_CREDENTIAL_GEMINI_FREE_2",
            "AI_GATEWAY_CREDENTIAL_GEMINI_FREE_3",
            "AI_GATEWAY_CREDENTIAL_GEMINI_FREE_4",
            "AI_GATEWAY_CREDENTIAL_GEMINI_DEFAULT",
            "GEMINI_FREE_TIER_API_KEY",
            "GEMINI_FREE_TIER_APIKEY",
            "GEMINI_API_KEY",
        ];
        unsafe {
            for name in clear {
                std::env::remove_var(name);
            }
        }
        let registry = CredentialRegistry::build(&providers());
        assert!(!registry.has_for(&InferenceProvider::GoogleGemini));
    }

    #[test]
    #[serial_test::serial(env)]
    fn legacy_default_synthesized_for_provider_with_single_env_key() {
        unsafe {
            std::env::remove_var("AI_GATEWAY_CREDENTIAL_DEEPSEEK_DEFAULT");
            std::env::set_var("DEEPSEEK_API_KEY", "deepseek-test");
        }
        let registry = CredentialRegistry::build(&providers());
        let deepseek = InferenceProvider::Named("deepseek".into());
        assert!(registry.has_for(&deepseek));
        let cred = registry.default_for(&deepseek).unwrap();
        assert_eq!(cred.id.0, "deepseek-default");
        unsafe {
            std::env::remove_var("DEEPSEEK_API_KEY");
        }
    }

    #[test]
    fn credential_id_display() {
        let id = ProviderCredentialId::new("gemini-free");
        assert_eq!(id.to_string(), "gemini-free");
        let parsed =
            ProviderCredentialId(CompactString::from_str("x").unwrap());
        assert_eq!(parsed.0, "x");
    }
}
