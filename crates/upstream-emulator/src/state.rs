use std::sync::Arc;

use ai_gateway::config::{
    provider_limits::ProviderLimitCatalog, providers::ProvidersConfig,
};

use crate::{
    catalog::ProviderTable, config::EmulatorConfig, limits::LimitRegistry,
    profiles::AdminProfiles, tier::CredentialTierMap,
};

#[derive(Clone)]
pub struct SharedState {
    pub config: EmulatorConfig,
    pub catalog: ProviderLimitCatalog,
    pub providers: ProvidersConfig,
    pub table: ProviderTable,
    pub limits: Arc<LimitRegistry>,
    pub profiles: AdminProfiles,
    pub tiers: CredentialTierMap,
}

impl SharedState {
    #[must_use]
    pub fn new(config: EmulatorConfig) -> Self {
        let catalog = ProviderLimitCatalog::default();
        let providers = ProvidersConfig::default();
        let table = ProviderTable::build(&providers, &catalog);
        Self {
            config,
            catalog,
            providers,
            table,
            limits: Arc::new(LimitRegistry::default()),
            profiles: AdminProfiles::default(),
            tiers: CredentialTierMap::load(),
        }
    }

    pub async fn sleep_for_usage(&self, provider: &str, total_tokens: u32) {
        let ms = self.config.latency_for(provider, total_tokens);
        if ms > 0 {
            tokio::time::sleep(std::time::Duration::from_millis(ms)).await;
        }
    }
}

#[derive(Debug, serde::Deserialize)]
pub struct ProfileRequest {
    pub scope: String,
    pub action: String,
}
