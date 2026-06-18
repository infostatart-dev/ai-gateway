//! Shared provider-limit slug resolution for gateway pacing and upstream
//! emulator.

use crate::{
    config::provider_limits::{
        ProviderLimitCatalog, ProviderLimitTier, QuotaLimits,
    },
    types::provider::InferenceProvider,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedModelLimits {
    pub tier: String,
    pub catalog_model: String,
    pub limits: QuotaLimits,
}

#[must_use]
pub fn normalize_model_slug(model: &str) -> String {
    model
        .rsplit('/')
        .next()
        .unwrap_or(model)
        .split(':')
        .next()
        .unwrap_or(model)
        .to_string()
}

#[must_use]
pub fn candidate_slugs(model: &str) -> Vec<String> {
    let mut out = Vec::new();
    if let Some((_, rest)) = model.split_once('/') {
        let trimmed = rest.split(':').next().unwrap_or(rest);
        out.push(trimmed.to_string());
    }
    let tail = normalize_model_slug(model);
    out.push(tail);
    for slug in out.clone() {
        if let Some(base) = slug.strip_suffix("-preview") {
            out.push(base.to_string());
        }
        if let Some(base) = slug.strip_suffix("-001") {
            out.push(base.to_string());
        }
    }
    out.sort_by_key(|slug| std::cmp::Reverse(slug.len()));
    out.dedup();
    out
}

#[must_use]
pub fn catalog_limit_resolve(
    catalog: &ProviderLimitCatalog,
    provider: &InferenceProvider,
    tier: &str,
    request_model: &str,
) -> Option<ResolvedModelLimits> {
    catalog_limit_resolve_with_key(catalog, provider, tier, request_model, None)
}

#[must_use]
pub fn catalog_limit_resolve_with_key(
    catalog: &ProviderLimitCatalog,
    provider: &InferenceProvider,
    tier: &str,
    request_model: &str,
    catalog_key: Option<&str>,
) -> Option<ResolvedModelLimits> {
    if let Some(key) = catalog_key {
        let config = catalog.provider(provider)?;
        if let Some(tier_cfg) = config.tier(tier)
            && let Some(model_entry) = tier_cfg.model(key)
        {
            return Some(ResolvedModelLimits {
                tier: tier.to_string(),
                catalog_model: key.to_string(),
                limits: model_entry.limits.clone(),
            });
        }
        for (tier_name, tier_cfg) in &config.tiers {
            if let Some(model_entry) = tier_cfg.model(key) {
                return Some(ResolvedModelLimits {
                    tier: tier_name.clone(),
                    catalog_model: key.to_string(),
                    limits: model_entry.limits.clone(),
                });
            }
        }
    }
    let config = catalog.provider(provider)?;
    if let Some(tier_cfg) = config.tiers.get(tier)
        && let Some(resolved) = resolve_in_tier(tier_cfg, tier, request_model)
    {
        return Some(resolved);
    }
    for (tier_name, tier_cfg) in &config.tiers {
        if let Some(resolved) =
            resolve_in_tier(tier_cfg, tier_name, request_model)
        {
            return Some(resolved);
        }
    }
    None
}

fn resolve_in_tier(
    tier: &ProviderLimitTier,
    tier_name: &str,
    request_model: &str,
) -> Option<ResolvedModelLimits> {
    let slug = normalize_model_slug(request_model);
    for candidate in candidate_slugs(request_model) {
        if let Some(model_entry) = tier.model(&candidate) {
            return Some(ResolvedModelLimits {
                tier: tier_name.to_string(),
                catalog_model: candidate,
                limits: model_entry.limits.clone(),
            });
        }
    }
    if slug != request_model
        && let Some(model_entry) = tier.model(&slug)
    {
        return Some(ResolvedModelLimits {
            tier: tier_name.to_string(),
            catalog_model: slug,
            limits: model_entry.limits.clone(),
        });
    }
    limits_from_rules(tier, request_model).map(|(matched_model, limits)| {
        ResolvedModelLimits {
            tier: tier_name.to_string(),
            catalog_model: matched_model,
            limits,
        }
    })
}

fn limits_from_rules(
    tier: &ProviderLimitTier,
    request_model: &str,
) -> Option<(String, QuotaLimits)> {
    for rule in tier.rules.values() {
        let Some(suffix) = rule.model_suffix.as_deref() else {
            continue;
        };
        if request_model.ends_with(suffix) {
            let matched = request_model
                .split_once('/')
                .map_or(request_model, |(_, rest)| rest)
                .strip_suffix(suffix)
                .unwrap_or(request_model)
                .to_string();
            return Some((matched, rule.limits.clone()));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        config::provider_limits::ProviderLimitCatalog,
        types::provider::InferenceProvider,
    };

    #[test]
    fn explicit_catalog_key_overrides_preview_strip() {
        let catalog = ProviderLimitCatalog::default();
        let provider = InferenceProvider::GoogleGemini;
        let resolved = catalog_limit_resolve_with_key(
            &catalog,
            &provider,
            "free",
            "gemini-3-flash-preview",
            Some("gemini-3-flash"),
        )
        .expect("explicit key");
        assert_eq!(resolved.catalog_model, "gemini-3-flash");
        assert_eq!(
            resolved.limits.rpd,
            catalog_limit_resolve(
                &catalog,
                &provider,
                "free",
                "gemini-3-flash-preview",
            )
            .expect("implicit")
            .limits
            .rpd
        );
    }

    #[test]
    fn preview_slug_maps_to_catalog_key() {
        let catalog = ProviderLimitCatalog::default();
        let provider = InferenceProvider::GoogleGemini;
        let resolved = catalog_limit_resolve(
            &catalog,
            &provider,
            "free",
            "gemini-3-flash-preview",
        )
        .expect("resolved");
        assert_eq!(resolved.catalog_model, "gemini-3-flash");
        assert_eq!(
            resolved.limits.rpd,
            catalog_limit_resolve(
                &catalog,
                &provider,
                "free",
                "gemini-3-flash",
            )
            .expect("bare")
            .limits
            .rpd
        );
    }

    #[test]
    fn openrouter_slash_model_resolves() {
        let catalog = ProviderLimitCatalog::default();
        let provider = InferenceProvider::OpenRouter;
        let resolved = catalog_limit_resolve(
            &catalog,
            &provider,
            "free",
            "openrouter/openai/gpt-oss-120b:free",
        )
        .expect("resolved");
        assert_eq!(resolved.catalog_model, "openai/gpt-oss-120b");
    }
}
