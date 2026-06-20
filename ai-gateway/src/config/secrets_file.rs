use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::RwLock,
};

use indexmap::IndexMap;
use serde::Deserialize;

use crate::types::{
    provider::{InferenceProvider, ProviderKey},
    secret::Secret,
};

pub const SECRETS_FILE_ENV: &str = "AI_GATEWAY_SECRETS_FILE";
const DEFAULT_LOCAL_SECRETS: &str = "dev/secrets.local.yaml";
const DEFAULT_USER_SECRETS: &str = ".config/ai-gateway/secrets.yaml";

static INSTALLED: RwLock<Option<SecretsFile>> = RwLock::new(None);

#[derive(Debug, Clone, Default)]
pub struct SecretsFile {
    pub path: Option<PathBuf>,
    pub base_dir: PathBuf,
    credentials: IndexMap<String, CredentialSecret>,
    pub integrations: Integrations,
    session_paths: HashMap<String, PathBuf>,
    cloudflare_account_ids: HashMap<String, String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct CredentialSecret {
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default)]
    pub api_key_file: Option<String>,
    #[serde(default)]
    pub session_file: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Integrations {
    #[serde(default)]
    pub helicone: Option<HeliconeSecret>,
    #[serde(default)]
    pub aws: Option<AwsSecret>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct HeliconeSecret {
    pub api_key: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct AwsSecret {
    pub access_key: String,
    pub secret_key: String,
    pub region: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
struct SecretsFileRaw {
    #[serde(default)]
    credentials: IndexMap<String, CredentialSecret>,
    #[serde(default)]
    integrations: Integrations,
}

impl SecretsFile {
    #[must_use]
    pub fn discover_path() -> Option<PathBuf> {
        if let Ok(path) = std::env::var(SECRETS_FILE_ENV) {
            let path = PathBuf::from(path);
            if path.is_file() {
                return Some(path);
            }
        }
        let local = PathBuf::from(DEFAULT_LOCAL_SECRETS);
        if local.is_file() {
            return Some(local);
        }
        std::env::var_os("HOME")
            .map(|home| PathBuf::from(home).join(DEFAULT_USER_SECRETS))
            .filter(|path| path.is_file())
    }

    #[must_use]
    pub fn load_discovered() -> Self {
        Self::discover_path()
            .and_then(|path| Self::load(&path).ok())
            .unwrap_or_default()
    }

    pub fn load(path: &Path) -> Result<Self, std::io::Error> {
        let raw = std::fs::read_to_string(path)?;
        let parsed: SecretsFileRaw =
            serde_yml::from_str(&raw).map_err(|e| {
                std::io::Error::new(std::io::ErrorKind::InvalidData, e)
            })?;
        let base_dir = path.parent().unwrap_or(Path::new(".")).to_path_buf();
        Ok(Self {
            path: Some(path.to_path_buf()),
            base_dir,
            credentials: parsed.credentials,
            integrations: parsed.integrations,
            session_paths: HashMap::new(),
            cloudflare_account_ids: HashMap::new(),
        })
    }

    pub fn install(secrets: Self) {
        *INSTALLED.write().expect("secrets install lock poisoned") =
            Some(secrets);
    }

    /// Holds the global secrets install lock for the duration of a unit test.
    #[cfg(any(test, feature = "testing"))]
    #[must_use]
    pub fn install_for_test(secrets: Self) -> InstalledSecretsGuard {
        InstalledSecretsGuard::install(secrets)
    }

    pub fn installed() -> Option<SecretsFile> {
        INSTALLED
            .read()
            .expect("secrets read lock poisoned")
            .clone()
    }

    #[must_use]
    pub fn session_path(credential_id: &str) -> Option<PathBuf> {
        Self::installed()?.session_paths.get(credential_id).cloned()
    }

    pub fn register_session_path(
        &mut self,
        credential_id: &str,
        path: PathBuf,
    ) {
        self.session_paths.insert(credential_id.to_string(), path);
    }

    #[must_use]
    pub fn cloudflare_account_id(credential_id: &str) -> Option<String> {
        Self::installed()?
            .cloudflare_account_ids
            .get(credential_id)
            .cloned()
    }

    #[must_use]
    pub fn credential(&self, id: &str) -> Option<&CredentialSecret> {
        self.credentials.get(id)
    }

    pub fn resolve_provider_key(
        &mut self,
        credential_id: &str,
        provider: &InferenceProvider,
    ) -> Option<ProviderKey> {
        let entry = self.credentials.get(credential_id)?.clone();
        if let Some(path) = entry.session_file.as_deref() {
            let resolved = resolve_path(&self.base_dir, path);
            if !session_path_valid(provider, &resolved) {
                return None;
            }
            self.session_paths
                .insert(credential_id.to_string(), resolved.clone());
            return Some(ProviderKey::Secret(Secret::from(
                resolved.display().to_string(),
            )));
        }
        let secret = entry.api_key.or_else(|| {
            read_file_secret(&self.base_dir, entry.api_key_file.as_deref())
        })?;
        if *provider == InferenceProvider::Named("cloudflare".into()) {
            let (account_id, token) =
                crate::config::cloudflare::parse_combined(&secret)?;
            self.cloudflare_account_ids
                .insert(credential_id.to_string(), account_id);
            return Some(ProviderKey::Secret(Secret::from(token)));
        }
        Some(ProviderKey::Secret(Secret::from(secret)))
    }
}

#[cfg(any(test, feature = "testing"))]
static TEST_INSTALL_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Serializes tests that mutate the process-wide [`SecretsFile`] install slot.
#[cfg(any(test, feature = "testing"))]
pub struct InstalledSecretsGuard {
    _lock: std::sync::MutexGuard<'static, ()>,
}

#[cfg(any(test, feature = "testing"))]
impl InstalledSecretsGuard {
    fn install(secrets: SecretsFile) -> Self {
        let lock = TEST_INSTALL_MUTEX
            .lock()
            .expect("secrets file test mutex poisoned");
        *INSTALLED.write().expect("secrets install lock poisoned") =
            Some(secrets);
        Self { _lock: lock }
    }
}

#[cfg(any(test, feature = "testing"))]
impl Drop for InstalledSecretsGuard {
    fn drop(&mut self) {
        *INSTALLED.write().expect("secrets install lock poisoned") = None;
    }
}

#[must_use]
pub fn resolve_path(base_dir: &Path, path: &str) -> PathBuf {
    let candidate = PathBuf::from(path);
    if candidate.is_absolute() {
        candidate
    } else {
        base_dir.join(candidate)
    }
}

fn read_file_secret(base_dir: &Path, path: Option<&str>) -> Option<String> {
    let path = path?;
    let content = std::fs::read_to_string(resolve_path(base_dir, path)).ok()?;
    let trimmed = content.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

fn session_path_valid(provider: &InferenceProvider, path: &Path) -> bool {
    if crate::config::chatgpt_web::is_chatgpt_web(provider) {
        return path.exists();
    }
    if crate::config::deepseek_web::is_deepseek_web(provider) {
        return crate::config::deepseek_web::session_valid(path);
    }
    if crate::config::perplexity_web::is_perplexity_web(provider) {
        return crate::config::perplexity_web::session_valid(path);
    }
    path.exists()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::provider::InferenceProvider;

    fn write_secrets(dir: &Path, yaml: &str) -> PathBuf {
        let path = dir.join("secrets.yaml");
        std::fs::write(&path, yaml).unwrap();
        path
    }

    #[test]
    fn resolve_relative_path_from_secrets_dir() {
        let base = PathBuf::from("/tmp/secrets-dir");
        assert_eq!(
            resolve_path(&base, "dev/key.txt"),
            PathBuf::from("/tmp/secrets-dir/dev/key.txt")
        );
    }

    #[test]
    fn loads_api_key_and_integrations() {
        let dir = std::env::temp_dir().join("ai-gw-secrets-load");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = write_secrets(
            &dir,
            r#"
credentials:
  openrouter-default:
    api-key: sk-or-test
integrations:
  helicone:
    api-key: sk-helicone-test
  aws:
    access-key: AKIA
    secret-key: secret
    region: eu-central-1
"#,
        );
        let secrets = SecretsFile::load(&path).unwrap();
        assert_eq!(
            secrets
                .credential("openrouter-default")
                .unwrap()
                .api_key
                .as_deref(),
            Some("sk-or-test")
        );
        assert_eq!(
            secrets.integrations.helicone.as_ref().unwrap().api_key,
            "sk-helicone-test"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn cloudflare_key_splits_account_and_token() {
        let dir = std::env::temp_dir().join("ai-gw-secrets-cf");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = write_secrets(
            &dir,
            "credentials:\n  cloudflare-default:\n    api-key: acct:cfut_tok\n",
        );
        let mut secrets = SecretsFile::load(&path).unwrap();
        let key = secrets
            .resolve_provider_key(
                "cloudflare-default",
                &InferenceProvider::Named("cloudflare".into()),
            )
            .unwrap();
        assert_eq!(key.as_secret().unwrap().expose(), "cfut_tok");
        assert_eq!(
            secrets
                .cloudflare_account_ids
                .get("cloudflare-default")
                .unwrap(),
            "acct"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn legacy_env_is_not_used_for_credentials() {
        unsafe {
            std::env::set_var(
                "AI_GATEWAY_CREDENTIAL_OPENROUTER_DEFAULT",
                "legacy",
            );
        }
        let dir = std::env::temp_dir().join("ai-gw-secrets-legacy");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = write_secrets(&dir, "credentials: {}\n");
        let mut secrets = SecretsFile::load(&path).unwrap();
        assert!(
            secrets
                .resolve_provider_key(
                    "openrouter-default",
                    &InferenceProvider::OpenRouter,
                )
                .is_none()
        );
        unsafe {
            std::env::remove_var("AI_GATEWAY_CREDENTIAL_OPENROUTER_DEFAULT");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}
