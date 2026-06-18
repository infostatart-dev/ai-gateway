use ai_gateway::{
    config::{
        catalog_limit_resolve::catalog_limit_resolve,
        provider_limits::ProviderLimitCatalog,
    },
    types::provider::InferenceProvider,
};

use super::bucket::ApiLimits;

#[must_use]
pub fn resolve_limits(
    catalog: &ProviderLimitCatalog,
    provider: &InferenceProvider,
    credential_tier: Option<&str>,
    request_model: &str,
) -> (String, String, ApiLimits) {
    if let Some(tier) = credential_tier
        && let Some(resolved) =
            catalog_limit_resolve(catalog, provider, tier, request_model)
    {
        let tier_limits = catalog
            .provider(provider)
            .and_then(|cfg| cfg.tier(tier))
            .map(|t| &t.limits);
        return (
            tier.to_string(),
            resolved.catalog_model,
            ApiLimits::from_quota(Some(&resolved.limits), tier_limits),
        );
    }
    if let Some(config) = catalog.provider(provider) {
        for (tier_name, tier) in &config.tiers {
            if let Some(resolved) = catalog_limit_resolve(
                catalog,
                provider,
                tier_name,
                request_model,
            ) {
                return (
                    tier_name.clone(),
                    resolved.catalog_model,
                    ApiLimits::from_quota(
                        Some(&resolved.limits),
                        Some(&tier.limits),
                    ),
                );
            }
        }
    }
    let slug = ai_gateway::config::catalog_limit_resolve::normalize_model_slug(
        request_model,
    );
    (
        String::from("default"),
        slug,
        ApiLimits::from_quota(None, None),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_groq_model_from_catalog() {
        let catalog = ProviderLimitCatalog::default();
        let provider = InferenceProvider::Named("groq".into());
        let (_, model, limits) = resolve_limits(
            &catalog,
            &provider,
            Some("free"),
            "llama-3.1-8b-instant",
        );
        assert_eq!(model, "llama-3.1-8b-instant");
        assert_eq!(limits.rpm, 30);
    }

    #[test]
    fn resolves_openrouter_slash_model() {
        let catalog = ProviderLimitCatalog::default();
        let provider = InferenceProvider::OpenRouter;
        let (_, model, _) = resolve_limits(
            &catalog,
            &provider,
            None,
            "openrouter/openai/gpt-oss-120b:free",
        );
        assert_eq!(model, "openai/gpt-oss-120b");
    }

    #[test]
    fn credential_tier_selects_limits() {
        let catalog = ProviderLimitCatalog::default();
        let provider = InferenceProvider::GoogleGemini;
        let (_, _, free) = resolve_limits(
            &catalog,
            &provider,
            Some("free"),
            "gemini-2.5-flash",
        );
        let (_, _, paid) = resolve_limits(
            &catalog,
            &provider,
            Some("tier-3"),
            "gemini-2.5-flash",
        );
        assert_ne!(free.rpm, paid.rpm);
    }

    #[test]
    fn gateway_emulator_fixture_parity() {
        use ai_gateway::config::provider_limits::QuotaValue;

        let catalog = ProviderLimitCatalog::default();
        let provider = InferenceProvider::GoogleGemini;
        for (model, expected_rpm, expected_rpd) in [
            ("gemini-3-flash-preview", 5_u32, 20_u32),
            ("gemini-3.5-flash-preview", 5, 20),
            ("gemini-3.1-flash-lite", 15, 500),
            ("gemini-2.5-flash", 5, 20),
            ("gemini-2.5-pro", 5, 50),
        ] {
            let gateway =
                catalog_limit_resolve(&catalog, &provider, "free", model)
                    .unwrap_or_else(|| panic!("gateway resolve {model}"));
            let (_, emu_model, emu_limits) =
                resolve_limits(&catalog, &provider, Some("free"), model);
            assert_eq!(gateway.catalog_model, emu_model);
            assert_eq!(
                gateway.limits.rpm,
                QuotaValue::Limited(u64::from(expected_rpm))
            );
            assert_eq!(
                gateway.limits.rpd,
                QuotaValue::Limited(u64::from(expected_rpd))
            );
            assert_eq!(emu_limits.rpm, expected_rpm);
            assert_eq!(emu_limits.rpd, Some(expected_rpd));
        }
    }
}
