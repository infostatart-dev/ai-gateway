mod bucket;
mod resolve;

#[cfg(test)]
mod tests;

use std::sync::{Arc, Mutex};

use ai_gateway::{
    config::provider_limits::ProviderLimitCatalog,
    types::provider::InferenceProvider,
};
pub use bucket::{ApiLimits, RateLimitVerdict, ScopeLimiter};
pub use resolve::resolve_limits;
use rustc_hash::FxHashMap;

pub struct ScopeLease<'a> {
    registry: &'a LimitRegistry,
    key: String,
}

impl Drop for ScopeLease<'_> {
    fn drop(&mut self) {
        self.registry.release_scope(&self.key);
    }
}

#[derive(Debug, Default)]
pub struct LimitRegistry {
    scopes: Mutex<FxHashMap<String, ScopeLimiter>>,
}

impl LimitRegistry {
    pub fn check_api_key(
        &self,
        catalog: &ProviderLimitCatalog,
        provider: &InferenceProvider,
        credential_tier: Option<&str>,
        request_model: &str,
        credential: &str,
        tokens: u32,
    ) -> Result<ScopeLease<'_>, RateLimitVerdict> {
        let limits =
            resolve_limits(catalog, provider, credential_tier, request_model).2;
        let key = format!("{provider}:{credential}");
        Self::check(self, key, limits, tokens)
    }

    fn check(
        &self,
        key: String,
        limits: ApiLimits,
        tokens: u32,
    ) -> Result<ScopeLease<'_>, RateLimitVerdict> {
        let mut scopes = self.scopes.lock().expect("limit registry");
        let scope = scopes
            .entry(key.clone())
            .or_insert_with(|| ScopeLimiter::new(limits));
        let verdict = scope.check_request(tokens);
        drop(scopes);
        if verdict == RateLimitVerdict::Allow {
            Ok(ScopeLease {
                registry: self,
                key,
            })
        } else {
            Err(verdict)
        }
    }

    pub fn release_scope(&self, key: &str) {
        if let Ok(mut scopes) = self.scopes.lock()
            && let Some(scope) = scopes.get_mut(key)
        {
            scope.release();
        }
    }

    pub fn reset(&self) {
        self.scopes.lock().expect("limit registry").clear();
    }

    pub fn snapshot(&self) -> Vec<(String, u32, u32, u32)> {
        self.scopes
            .lock()
            .expect("limit registry")
            .iter()
            .map(|(k, v)| (k.clone(), v.rpm_used(), v.tpm_used(), v.rpd_used()))
            .collect()
    }
}

pub type SharedLimits = Arc<LimitRegistry>;
