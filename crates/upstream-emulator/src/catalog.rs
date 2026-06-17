use ai_gateway::{
    config::{
        provider_limits::ProviderLimitCatalog, providers::ProvidersConfig,
    },
    types::provider::InferenceProvider,
};

use crate::family::{ProtocolFamily, protocol_family};

#[derive(Debug, Clone)]
pub struct ProviderEntry {
    pub id: InferenceProvider,
    pub family: ProtocolFamily,
}

#[derive(Debug, Clone)]
pub struct ProviderTable {
    pub entries: Vec<ProviderEntry>,
}

impl ProviderTable {
    #[must_use]
    pub fn build(
        providers: &ProvidersConfig,
        limits: &ProviderLimitCatalog,
    ) -> Self {
        let entries = providers
            .iter()
            .filter(|(id, _)| !is_browser_session(id, limits))
            .map(|(id, cfg)| ProviderEntry {
                id: id.clone(),
                family: protocol_family(cfg),
            })
            .collect();
        Self { entries }
    }
}

fn is_browser_session(
    provider: &InferenceProvider,
    limits: &ProviderLimitCatalog,
) -> bool {
    limits.provider(provider).and_then(|c| c.scope.as_deref())
        == Some("browser-session")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skips_browser_session_providers() {
        let providers = ProvidersConfig::default();
        let limits = ProviderLimitCatalog::default();
        let table = ProviderTable::build(&providers, &limits);
        assert!(
            !table
                .entries
                .iter()
                .any(|e| e.id.to_string().ends_with("-web"))
        );
    }

    #[test]
    fn mounts_every_api_key_provider_from_embedded_catalog() {
        let providers = ProvidersConfig::default();
        let limits = ProviderLimitCatalog::default();
        let api_count = providers
            .iter()
            .filter(|(id, _)| !is_browser_session(id, &limits))
            .count();
        let table = ProviderTable::build(&providers, &limits);
        assert_eq!(table.entries.len(), api_count);
    }
}
