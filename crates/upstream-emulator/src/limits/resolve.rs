use ai_gateway::{
    config::provider_limits::ProviderLimitCatalog,
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
    if let Some(config) = catalog.provider(provider) {
        if let Some(tier_name) = credential_tier
            && let Some(tier) = config.tiers.get(tier_name)
            && let Some(resolved) =
                resolve_in_tier(tier, tier_name, request_model)
        {
            return resolved;
        }
        for (tier_name, tier) in &config.tiers {
            if let Some(resolved) =
                resolve_in_tier(tier, tier_name, request_model)
            {
                return resolved;
            }
        }
    }
    let slug = normalize_model_slug(request_model);
    (
        String::from("default"),
        slug,
        ApiLimits::from_quota(None, None),
    )
}

fn resolve_in_tier(
    tier: &ai_gateway::config::provider_limits::ProviderLimitTier,
    tier_name: &str,
    request_model: &str,
) -> Option<(String, String, ApiLimits)> {
    let slug = normalize_model_slug(request_model);
    for candidate in candidate_slugs(request_model) {
        if let Some(model_entry) = tier.model(&candidate) {
            return Some((
                tier_name.to_string(),
                candidate,
                ApiLimits::from_quota(
                    Some(&model_entry.limits),
                    Some(&tier.limits),
                ),
            ));
        }
    }
    if slug != request_model
        && let Some(model_entry) = tier.model(&slug)
    {
        return Some((
            tier_name.to_string(),
            slug.clone(),
            ApiLimits::from_quota(
                Some(&model_entry.limits),
                Some(&tier.limits),
            ),
        ));
    }
    if let Some((matched_model, limits)) =
        limits_from_rules(tier, request_model)
    {
        return Some((
            tier_name.to_string(),
            matched_model,
            ApiLimits::from_quota(Some(&limits), Some(&tier.limits)),
        ));
    }
    None
}

fn candidate_slugs(model: &str) -> Vec<String> {
    let mut out = Vec::new();
    if let Some((_, rest)) = model.split_once('/') {
        let trimmed = rest.split(':').next().unwrap_or(rest);
        out.push(trimmed.to_string());
    }
    let tail = model
        .rsplit('/')
        .next()
        .unwrap_or(model)
        .split(':')
        .next()
        .unwrap_or(model);
    out.push(tail.to_string());
    out.sort_by_key(|slug| std::cmp::Reverse(slug.len()));
    out.dedup();
    out
}

fn limits_from_rules(
    tier: &ai_gateway::config::provider_limits::ProviderLimitTier,
    request_model: &str,
) -> Option<(String, ai_gateway::config::provider_limits::QuotaLimits)> {
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

fn normalize_model_slug(model: &str) -> String {
    model
        .rsplit('/')
        .next()
        .unwrap_or(model)
        .split(':')
        .next()
        .unwrap_or(model)
        .to_string()
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
}
