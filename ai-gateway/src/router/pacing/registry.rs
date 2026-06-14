use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use super::{gate::PacingGate, limits::PacingLimits};
use crate::{
    config::chatgpt_web::{is_chatgpt_web, session_path_from_env},
    config::provider_limits::ProviderLimitCatalog,
    types::provider::InferenceProvider,
};

/// Factory + cache: one [`PacingGate`] per `(provider, scope key)` (Registry pattern).
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
    pub fn limits_for(&self, provider: &InferenceProvider) -> Option<PacingLimits> {
        self.catalog.pacing_limits_for(provider)
    }

    pub fn gate_for(&self, provider: &InferenceProvider) -> Option<Arc<PacingGate>> {
        let limits = self.limits_for(provider)?;
        let key = (provider.to_string(), gate_scope_key(provider));
        let mut gates = self
            .gates
            .lock()
            .expect("pacing registry mutex poisoned");
        Some(
            gates
                .entry(key)
                .or_insert_with(|| Arc::new(PacingGate::new(limits.clone())))
                .clone(),
        )
    }
}

fn gate_scope_key(provider: &InferenceProvider) -> String {
    if is_chatgpt_web(provider) {
        session_path_from_env()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "missing-session".into())
    } else {
        "default".into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::provider_limits::ProviderLimitCatalog;

    #[test]
    fn api_provider_scope_is_default_bucket() {
        let key = gate_scope_key(&InferenceProvider::OpenAI);
        assert_eq!(key, "default");
    }

    #[test]
    fn registry_reuses_gate_per_provider_scope() {
        let registry = PacingRegistry::new(ProviderLimitCatalog::default());
        let provider = InferenceProvider::Named("chatgpt-web".into());
        let a = registry.gate_for(&provider).expect("gate");
        let b = registry.gate_for(&provider).expect("gate");
        assert!(Arc::ptr_eq(&a, &b));
    }
}
