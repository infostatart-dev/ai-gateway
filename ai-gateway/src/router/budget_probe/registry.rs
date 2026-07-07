use std::{
    collections::HashMap,
    sync::Mutex,
    time::{Duration, Instant},
};

use reqwest::header::{AUTHORIZATION, HeaderValue};

use super::{parse::parse_openrouter_key_info, snapshot::KeyInfoSnapshot};
use crate::{
    config::{
        credentials::{CredentialRegistry, ProviderCredentialId},
        provider_limits::{ProviderLimitCatalog, RuntimeLimitSource},
    },
    types::{
        provider::{InferenceProvider, ProviderKey},
        secret::Secret,
    },
};

const PROBE_TTL: Duration = Duration::from_mins(5);

pub async fn fetch_key_info(
    client: &reqwest::Client,
    source: &RuntimeLimitSource,
    api_key: &str,
) -> Option<KeyInfoSnapshot> {
    let url = source.url.as_deref()?;
    let mut req = client.get(url);
    if let Ok(value) = HeaderValue::from_str(&format!("Bearer {api_key}")) {
        req = req.header(AUTHORIZATION, value);
    }
    let body = req.send().await.ok()?.bytes().await.ok()?;
    parse_openrouter_key_info(&body)
}

#[derive(Debug)]
pub struct BudgetProbeRegistry {
    client: reqwest::Client,
    catalog: ProviderLimitCatalog,
    cache: Mutex<HashMap<(String, String), KeyInfoSnapshot>>,
}

impl BudgetProbeRegistry {
    #[must_use]
    pub fn new(catalog: ProviderLimitCatalog) -> Self {
        Self {
            client: reqwest::Client::new(),
            catalog,
            cache: Mutex::new(HashMap::new()),
        }
    }

    pub async fn should_skip_candidate(
        &self,
        credentials: &CredentialRegistry,
        provider: &InferenceProvider,
        credential_id: &ProviderCredentialId,
        model: &str,
    ) -> bool {
        let cache_key = (provider.to_string(), credential_id.to_string());
        if let Some(snapshot) = self.cached(&cache_key)
            && snapshot.blocks_paid_route(model)
        {
            return true;
        }
        let Some(source) = self.key_info_source(provider) else {
            return false;
        };
        let Some(credential) = credentials.get(credential_id) else {
            return false;
        };
        let Some(api_key) = secret_key(&credential.key) else {
            return false;
        };
        let Some(snapshot) =
            fetch_key_info(&self.client, source, api_key.expose()).await
        else {
            tracing::warn!(
                provider = %provider,
                credential = %credential_id,
                "budget probe failed; fail-open"
            );
            return false;
        };
        let skip = snapshot.blocks_paid_route(model);
        self.store(cache_key, snapshot);
        skip
    }

    pub fn record_payment_required(
        &self,
        provider: &InferenceProvider,
        credential_id: &ProviderCredentialId,
    ) {
        let cache_key = (provider.to_string(), credential_id.to_string());
        self.store(
            cache_key,
            KeyInfoSnapshot {
                limit_remaining: Some(0.0),
                is_free_tier: false,
                probed_at: Instant::now(),
            },
        );
    }

    fn key_info_source(
        &self,
        provider: &InferenceProvider,
    ) -> Option<&RuntimeLimitSource> {
        self.catalog
            .provider(provider)?
            .runtime_sources
            .get("key-info")
    }

    fn cached(&self, key: &(String, String)) -> Option<KeyInfoSnapshot> {
        let cache = self.cache.lock().ok()?;
        let snap = cache.get(key)?;
        if snap.probed_at.elapsed() > PROBE_TTL {
            return None;
        }
        Some(*snap)
    }

    fn store(&self, key: (String, String), snapshot: KeyInfoSnapshot) {
        if let Ok(mut cache) = self.cache.lock() {
            cache.insert(key, snapshot);
        }
    }
}

fn secret_key(key: &ProviderKey) -> Option<&Secret<String>> {
    key.as_secret()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        config::{
            credentials::ProviderCredentialId,
            provider_limits::ProviderLimitCatalog,
        },
        types::provider::InferenceProvider,
    };

    #[tokio::test]
    async fn record_payment_required_blocks_paid_but_not_free_routes() {
        let app_state = crate::app_state::AppState::test_default().await;
        let registry =
            BudgetProbeRegistry::new(ProviderLimitCatalog::default());
        let provider = InferenceProvider::OpenRouter;
        let credential = ProviderCredentialId::new("openrouter-default");
        registry.record_payment_required(&provider, &credential);
        let credentials = app_state.config().credentials.clone();
        assert!(
            !registry
                .should_skip_candidate(
                    &credentials,
                    &provider,
                    &credential,
                    "openai/gpt-oss-120b:free",
                )
                .await
        );
        assert!(
            registry
                .should_skip_candidate(
                    &credentials,
                    &provider,
                    &credential,
                    "openai/gpt-4o-mini",
                )
                .await
        );
    }
}
