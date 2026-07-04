use std::{collections::HashMap, fmt, str::FromStr, sync::Arc};

use indexmap::IndexSet;
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    config::client_access::{
        ClientAccessKeyStatus, ClientAccessLimitsConfig,
        ClientAccessRegistryFile,
    },
    types::{
        org::OrgId, provider::InferenceProvider, router::RouterId, user::UserId,
    },
};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ClientAccessKeyHash(String);

impl ClientAccessKeyHash {
    pub const PREFIX: &'static str = "sha256:";

    pub fn parse(value: &str) -> Result<Self, ClientAccessSnapshotError> {
        let Some(hex) = value.strip_prefix(Self::PREFIX) else {
            return Err(ClientAccessSnapshotError::InvalidHashFormat(
                value.to_string(),
            ));
        };
        if hex.len() != 64 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(ClientAccessSnapshotError::InvalidHashFormat(
                value.to_string(),
            ));
        }
        Ok(Self(format!(
            "{}{}",
            Self::PREFIX,
            hex.to_ascii_lowercase()
        )))
    }

    #[must_use]
    pub fn from_bearer_token(token: &str) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(token.as_bytes());
        let digest = hasher.finalize();
        let hex =
            digest.iter().fold(String::with_capacity(64), |mut out, b| {
                use std::fmt::Write as _;
                let _ = write!(out, "{b:02x}");
                out
            });
        Self(format!("{}{}", Self::PREFIX, hex))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ClientAccessKeyHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ClientAccessScope {
    All,
    UnifiedApi,
    Router(RouterId),
    Direct(InferenceProvider),
}

impl FromStr for ClientAccessScope {
    type Err = ClientAccessSnapshotError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        if value == "*" {
            return Ok(Self::All);
        }
        if value == "unified-api" {
            return Ok(Self::UnifiedApi);
        }
        if let Some(id) = value.strip_prefix("router:") {
            if id.is_empty() {
                return Err(ClientAccessSnapshotError::InvalidScope(
                    value.to_string(),
                ));
            }
            return Ok(Self::Router(RouterId::Named(id.into())));
        }
        if let Some(provider) = value.strip_prefix("direct:") {
            if provider.is_empty() {
                return Err(ClientAccessSnapshotError::InvalidScope(
                    value.to_string(),
                ));
            }
            let provider = InferenceProvider::from_str(provider)
                .expect("inference provider parsing is infallible");
            return Ok(Self::Direct(provider));
        }
        Err(ClientAccessSnapshotError::InvalidScope(value.to_string()))
    }
}

impl ClientAccessScope {
    #[must_use]
    pub fn allows(
        &self,
        request_kind: &crate::types::extensions::RequestKind,
        router_id: Option<&RouterId>,
        direct_provider: Option<&InferenceProvider>,
    ) -> bool {
        match self {
            Self::All => true,
            Self::UnifiedApi => matches!(
                request_kind,
                crate::types::extensions::RequestKind::UnifiedApi
            ),
            Self::Router(allowed) => {
                matches!(
                    request_kind,
                    crate::types::extensions::RequestKind::Router
                ) && router_id == Some(allowed)
            }
            Self::Direct(allowed) => {
                matches!(
                    request_kind,
                    crate::types::extensions::RequestKind::DirectProxy
                        | crate::types::extensions::RequestKind::Managed
                ) && direct_provider == Some(allowed)
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct ClientAccessSnapshotSubject {
    pub id: Arc<str>,
    pub org_id: OrgId,
    pub user_id: UserId,
}

#[derive(Debug, Clone)]
pub struct ClientAccessSnapshotPlan {
    pub id: Arc<str>,
    pub max_output_tokens: u32,
    pub limits: ClientAccessLimitsConfig,
}

#[derive(Debug, Clone)]
pub struct ClientAccessSnapshotKey {
    pub id: Arc<str>,
    pub hash: ClientAccessKeyHash,
    pub subject: Arc<ClientAccessSnapshotSubject>,
    pub plan: Arc<ClientAccessSnapshotPlan>,
    pub status: ClientAccessKeyStatus,
    pub expires_at: Option<chrono::DateTime<chrono::Utc>>,
    pub scopes: Arc<[ClientAccessScope]>,
}

impl ClientAccessSnapshotKey {
    #[must_use]
    pub fn is_active_at(&self, now: chrono::DateTime<chrono::Utc>) -> bool {
        self.status == ClientAccessKeyStatus::Active
            && self.expires_at.is_none_or(|expires_at| expires_at > now)
    }

    #[must_use]
    pub fn allows(
        &self,
        request_kind: &crate::types::extensions::RequestKind,
        router_id: Option<&RouterId>,
        direct_provider: Option<&InferenceProvider>,
    ) -> bool {
        self.scopes
            .iter()
            .any(|scope| scope.allows(request_kind, router_id, direct_provider))
    }
}

#[derive(Debug, Clone, Default)]
pub struct ClientAccessSnapshot {
    keys_by_hash: HashMap<ClientAccessKeyHash, Arc<ClientAccessSnapshotKey>>,
    keys_by_id: HashMap<Arc<str>, Arc<ClientAccessSnapshotKey>>,
}

impl ClientAccessSnapshot {
    pub fn from_registry(
        registry: ClientAccessRegistryFile,
    ) -> Result<Self, ClientAccessSnapshotError> {
        if registry.version != 1 {
            return Err(ClientAccessSnapshotError::UnsupportedVersion(
                registry.version,
            ));
        }
        if registry.plans.is_empty() {
            return Err(ClientAccessSnapshotError::NoPlans);
        }
        validate_plan_limits(&registry)?;

        let subjects = registry
            .subjects
            .into_iter()
            .map(|(id, subject)| {
                let id: Arc<str> = Arc::from(id);
                (
                    id.clone(),
                    Arc::new(ClientAccessSnapshotSubject {
                        id,
                        org_id: subject.org_id,
                        user_id: subject.user_id,
                    }),
                )
            })
            .collect::<HashMap<_, _>>();
        let plans = registry
            .plans
            .into_iter()
            .map(|(id, plan)| {
                let id: Arc<str> = Arc::from(id);
                (
                    id.clone(),
                    Arc::new(ClientAccessSnapshotPlan {
                        id,
                        max_output_tokens: plan.max_output_tokens,
                        limits: plan.limits,
                    }),
                )
            })
            .collect::<HashMap<_, _>>();

        let mut keys_by_hash = HashMap::new();
        let mut keys_by_id = HashMap::new();
        for (key_id, key) in registry.keys {
            let hash = ClientAccessKeyHash::parse(&key.hash)?;
            if key.scopes.is_empty() {
                return Err(ClientAccessSnapshotError::EmptyScopes(key_id));
            }
            let subject = subjects
                .get(key.subject.as_str())
                .cloned()
                .ok_or_else(|| ClientAccessSnapshotError::UnknownSubject {
                    key_id: key_id.clone(),
                    subject_id: key.subject.clone(),
                })?;
            let plan =
                plans.get(key.plan.as_str()).cloned().ok_or_else(|| {
                    ClientAccessSnapshotError::UnknownPlan {
                        key_id: key_id.clone(),
                        plan_id: key.plan.clone(),
                    }
                })?;
            let scopes = parse_scopes(&key_id, &key.scopes)?;
            let id: Arc<str> = Arc::from(key_id);
            let snapshot_key = Arc::new(ClientAccessSnapshotKey {
                id: id.clone(),
                hash: hash.clone(),
                subject,
                plan,
                status: key.status,
                expires_at: key.expires_at,
                scopes,
            });
            if keys_by_hash
                .insert(hash.clone(), snapshot_key.clone())
                .is_some()
            {
                return Err(ClientAccessSnapshotError::DuplicateHash(hash));
            }
            keys_by_id.insert(id, snapshot_key);
        }
        Ok(Self {
            keys_by_hash,
            keys_by_id,
        })
    }

    #[must_use]
    pub fn lookup_hash(
        &self,
        hash: &ClientAccessKeyHash,
    ) -> Option<Arc<ClientAccessSnapshotKey>> {
        self.keys_by_hash.get(hash).cloned()
    }

    #[must_use]
    pub fn lookup_bearer_token(
        &self,
        token: &str,
    ) -> Option<Arc<ClientAccessSnapshotKey>> {
        self.lookup_hash(&ClientAccessKeyHash::from_bearer_token(token))
    }

    #[must_use]
    pub fn lookup_key_id(
        &self,
        key_id: &str,
    ) -> Option<Arc<ClientAccessSnapshotKey>> {
        self.keys_by_id.get(key_id).cloned()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.keys_by_id.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.keys_by_id.is_empty()
    }
}

fn parse_scopes(
    key_id: &str,
    values: &IndexSet<String>,
) -> Result<Arc<[ClientAccessScope]>, ClientAccessSnapshotError> {
    values
        .iter()
        .map(|value| {
            value.parse::<ClientAccessScope>().map_err(|err| {
                ClientAccessSnapshotError::InvalidKeyScope {
                    key_id: key_id.to_string(),
                    scope: value.clone(),
                    source: Box::new(err),
                }
            })
        })
        .collect::<Result<Vec<_>, _>>()
        .map(Arc::from)
}

fn validate_plan_limits(
    registry: &ClientAccessRegistryFile,
) -> Result<(), ClientAccessSnapshotError> {
    for (plan_id, plan) in &registry.plans {
        validate_window_limits(plan_id, "requests", &plan.limits.requests)?;
        validate_window_limits(plan_id, "tokens", &plan.limits.tokens)?;
    }
    Ok(())
}

fn validate_window_limits(
    plan_id: &str,
    family: &'static str,
    limits: &crate::config::client_access::ClientAccessWindowLimitsConfig,
) -> Result<(), ClientAccessSnapshotError> {
    for (name, value) in [
        ("per-minute", limits.per_minute),
        ("per-day", limits.per_day),
        ("per-week", limits.per_week),
    ] {
        if value == Some(0) {
            return Err(ClientAccessSnapshotError::ZeroLimit {
                plan_id: plan_id.to_string(),
                family,
                name,
            });
        }
    }
    Ok(())
}

#[derive(Debug, Error)]
pub enum ClientAccessSnapshotError {
    #[error("unsupported client access registry version {0}")]
    UnsupportedVersion(u16),
    #[error("client access registry must define at least one plan")]
    NoPlans,
    #[error("invalid inbound key hash format `{0}`")]
    InvalidHashFormat(String),
    #[error("duplicate inbound key hash `{0}`")]
    DuplicateHash(ClientAccessKeyHash),
    #[error("key `{key_id}` references unknown subject `{subject_id}`")]
    UnknownSubject { key_id: String, subject_id: String },
    #[error("key `{key_id}` references unknown plan `{plan_id}`")]
    UnknownPlan { key_id: String, plan_id: String },
    #[error("key `{0}` must declare at least one scope")]
    EmptyScopes(String),
    #[error("invalid client access scope `{0}`")]
    InvalidScope(String),
    #[error("key `{key_id}` has invalid scope `{scope}`: {source}")]
    InvalidKeyScope {
        key_id: String,
        scope: String,
        source: Box<ClientAccessSnapshotError>,
    },
    #[error("plan `{plan_id}` has zero {family}.{name} limit")]
    ZeroLimit {
        plan_id: String,
        family: &'static str,
        name: &'static str,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::client_access::ClientAccessRegistryFile;

    fn parse_snapshot(yaml: &str) -> ClientAccessSnapshot {
        let registry: ClientAccessRegistryFile =
            serde_yml::from_str(yaml).unwrap();
        ClientAccessSnapshot::from_registry(registry).unwrap()
    }

    fn key_hash(raw: &str) -> String {
        ClientAccessKeyHash::from_bearer_token(raw).to_string()
    }

    #[test]
    fn hash_parser_requires_sha256_prefix() {
        assert!(ClientAccessKeyHash::parse("abc").is_err());
        assert!(
            ClientAccessKeyHash::parse(
                "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
            )
            .is_ok()
        );
    }

    #[test]
    fn snapshot_resolves_key_subject_plan_and_scopes() {
        let hash = key_hash("sk-test");
        let yaml = format!(
            r#"
version: 1
subjects:
  acme:
    org-id: "00000000-0000-0000-0000-000000000001"
    user-id: "00000000-0000-0000-0000-000000000002"
plans:
  starter:
    limits:
      requests:
        per-minute: 1
      tokens:
        per-day: 100
keys:
  acme-main:
    hash: "{hash}"
    subject: acme
    status: active
    plan: starter
    scopes:
      - unified-api
      - router:autodefault
"#
        );
        let snapshot = parse_snapshot(&yaml);
        let key = snapshot.lookup_bearer_token("sk-test").unwrap();
        assert_eq!(key.id.as_ref(), "acme-main");
        assert_eq!(key.plan.id.as_ref(), "starter");
        assert_eq!(snapshot.len(), 1);
    }

    #[test]
    fn unknown_plan_is_rejected() {
        let hash = key_hash("sk-test");
        let yaml = format!(
            r#"
version: 1
subjects:
  acme:
    org-id: "00000000-0000-0000-0000-000000000001"
    user-id: "00000000-0000-0000-0000-000000000002"
plans:
  starter:
    limits:
      requests:
        per-minute: 1
      tokens:
        per-day: 100
keys:
  acme-main:
    hash: "{hash}"
    subject: acme
    status: active
    plan: missing
    scopes:
      - unified-api
"#
        );
        let registry: ClientAccessRegistryFile =
            serde_yml::from_str(&yaml).unwrap();
        assert!(matches!(
            ClientAccessSnapshot::from_registry(registry),
            Err(ClientAccessSnapshotError::UnknownPlan { .. })
        ));
    }
}
