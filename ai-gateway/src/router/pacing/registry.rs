use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use super::{
    gate::PacingGate,
    limits::PacingLimits,
    scope::{pacing_scope_key, resolve_pacing_scope},
};
use crate::{
    config::{
        credentials::ProviderCredentialId,
        provider_limits::{ProviderLimitCatalog, ProviderQuotaProfile},
    },
    types::provider::InferenceProvider,
};

/// Factory + cache: one [`PacingGate`] per `(provider, account scope[,
/// model])`.
#[derive(Debug)]
pub struct PacingRegistry {
    gates: Mutex<HashMap<(String, String), Arc<PacingGate>>>,
    catalog: ProviderLimitCatalog,
}

impl PacingRegistry {
    #[must_use]
    pub fn new(catalog: ProviderLimitCatalog) -> Self {
        Self {
            gates: Mutex::new(HashMap::new()),
            catalog,
        }
    }

    #[must_use]
    pub fn limits_for(
        &self,
        provider: &InferenceProvider,
    ) -> Option<PacingLimits> {
        self.catalog.pacing_limits_for(provider)
    }

    pub fn gate_for(
        &self,
        provider: &InferenceProvider,
        credential_id: Option<&ProviderCredentialId>,
        tier: Option<&str>,
        model: Option<&str>,
    ) -> Option<Arc<PacingGate>> {
        let quota_profile = self.catalog.quota_profile(provider);
        let limits = if quota_profile == ProviderQuotaProfile::PerModel {
            let tier = tier?;
            let model = model?;
            PacingLimits::resolve_for_model(
                &self.catalog,
                provider,
                tier,
                model,
            )?
        } else {
            self.limits_for(provider)?
        };
        let scope =
            resolve_pacing_scope(provider, credential_id, model, quota_profile);
        let key = (provider.to_string(), pacing_scope_key(&scope));
        let mut gates =
            self.gates.lock().expect("pacing registry mutex poisoned");
        Some(
            gates
                .entry(key)
                .or_insert_with(|| Arc::new(PacingGate::new(limits)))
                .clone(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{
        credentials::ProviderCredentialId,
        provider_limits::ProviderLimitCatalog,
    };

    #[test]
    fn registry_reuses_gate_for_same_credential_scope() {
        let registry = PacingRegistry::new(ProviderLimitCatalog::default());
        let provider = InferenceProvider::Named("chatgpt-web".into());
        let gate_a = registry
            .gate_for(&provider, None, Some("plus-single-session"), None)
            .expect("chatgpt-web pacing");
        let gate_b = registry
            .gate_for(&provider, None, Some("plus-single-session"), None)
            .expect("gate");
        assert!(Arc::ptr_eq(&gate_a, &gate_b));
    }

    #[test]
    fn per_model_registry_isolates_gates_by_model() {
        let registry = PacingRegistry::new(ProviderLimitCatalog::default());
        let provider = InferenceProvider::GoogleGemini;
        let cred = ProviderCredentialId::new("gemini-free-8");
        let gate_a = registry
            .gate_for(
                &provider,
                Some(&cred),
                Some("free"),
                Some("gemini-3-flash-preview"),
            )
            .expect("flash gate");
        let gate_b = registry
            .gate_for(
                &provider,
                Some(&cred),
                Some("free"),
                Some("gemini-3.1-flash-lite"),
            )
            .expect("lite gate");
        assert!(!Arc::ptr_eq(&gate_a, &gate_b));
    }

    #[test]
    #[serial_test::serial]
    fn registry_isolates_gates_by_credential_scope() {
        let path_a = std::env::temp_dir().join("ai-gw-pacing-a.json");
        let path_b = std::env::temp_dir().join("ai-gw-pacing-b.json");
        std::fs::write(&path_a, r#"{"cookie":"a"}"#).unwrap();
        std::fs::write(&path_b, r#"{"cookie":"b"}"#).unwrap();

        let mut secrets = crate::config::secrets_file::SecretsFile::default();
        secrets.register_session_path("chatgpt-web-a", path_a.clone());
        secrets.register_session_path("chatgpt-web-b", path_b.clone());
        let _guard =
            crate::config::secrets_file::SecretsFile::install_for_test(secrets);

        let registry = PacingRegistry::new(ProviderLimitCatalog::default());
        let provider = InferenceProvider::Named("chatgpt-web".into());
        let cred_a = ProviderCredentialId::new("chatgpt-web-a");
        let cred_b = ProviderCredentialId::new("chatgpt-web-b");
        let gate_a = registry
            .gate_for(
                &provider,
                Some(&cred_a),
                Some("plus-single-session"),
                None,
            )
            .expect("gate a");
        let gate_b = registry
            .gate_for(
                &provider,
                Some(&cred_b),
                Some("plus-single-session"),
                None,
            )
            .expect("gate b");
        assert!(!Arc::ptr_eq(&gate_a, &gate_b));

        let _ = std::fs::remove_file(path_a);
        let _ = std::fs::remove_file(path_b);
    }
}
