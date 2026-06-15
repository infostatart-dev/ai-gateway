use std::{borrow::Cow, collections::HashSet, hash::BuildHasher};

use crate::types::{
    model_id::{ModelId, ModelIdWithoutVersion},
    provider::InferenceProvider,
};

/// Resolve a source model to an `OpenRouter` slug registered in
/// `providers.yaml`.
///
/// Namespace is derived from the source provider. Only vendors whose gateway id
/// differs from `OpenRouter`'s slug need an entry in `NAMESPACE_OVERRIDES`;
/// every other `Named` provider passes through its id. The slug must exist in
/// the configured `OpenRouter` model list — no hand-maintained vendor
/// whitelist.
#[must_use]
pub fn resolve_upstream_model<S: BuildHasher>(
    source_model: &ModelId,
    offered: &HashSet<ModelIdWithoutVersion, S>,
) -> Option<ModelId> {
    let provider = source_model.inference_provider()?;
    namespace_candidates(&provider)
        .into_iter()
        .find_map(|namespace| try_slug(source_model, &namespace, offered))
}

fn try_slug<S: BuildHasher>(
    source_model: &ModelId,
    namespace: &str,
    offered: &HashSet<ModelIdWithoutVersion, S>,
) -> Option<ModelId> {
    let candidate = ModelId::from_str_and_provider(
        InferenceProvider::OpenRouter,
        &format!("{namespace}/{source_model}"),
    )
    .ok()?;
    offered
        .contains(&ModelIdWithoutVersion::from(candidate.clone()))
        .then_some(candidate)
}

fn namespace_candidates(
    provider: &InferenceProvider,
) -> Vec<Cow<'static, str>> {
    match provider {
        InferenceProvider::OpenAI => vec![Cow::Borrowed("openai")],
        InferenceProvider::Anthropic => vec![Cow::Borrowed("anthropic")],
        InferenceProvider::GoogleGemini => vec![Cow::Borrowed("google")],
        InferenceProvider::Bedrock
        | InferenceProvider::Ollama
        | InferenceProvider::OpenRouter => Vec::new(),
        InferenceProvider::Named(name) => match name.as_str() {
            "xai" => vec![Cow::Borrowed("x-ai")],
            "mistral" => vec![Cow::Borrowed("mistralai")],
            _ => vec![Cow::Owned(name.to_string())],
        },
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use rustc_hash::FxHashSet;

    use super::*;

    fn offered(slugs: &[&str]) -> FxHashSet<ModelIdWithoutVersion> {
        slugs
            .iter()
            .map(|slug| {
                ModelIdWithoutVersion::from(
                    ModelId::from_str_and_provider(
                        InferenceProvider::OpenRouter,
                        slug,
                    )
                    .unwrap(),
                )
            })
            .collect()
    }

    #[test]
    fn resolves_when_slug_is_in_catalog() {
        let source = ModelId::from_str("openai/gpt-4o-mini").unwrap();
        let catalog = offered(&["openai/gpt-4o-mini"]);
        let resolved = resolve_upstream_model(&source, &catalog).unwrap();
        assert_eq!(resolved.to_string(), "openai/gpt-4o-mini");
    }

    #[test]
    fn applies_namespace_override_for_xai() {
        let source = ModelId::from_str("xai/grok-4").unwrap();
        let catalog = offered(&["x-ai/grok-4"]);
        let resolved = resolve_upstream_model(&source, &catalog).unwrap();
        assert_eq!(resolved.to_string(), "x-ai/grok-4");
    }

    #[test]
    fn applies_namespace_override_for_mistral() {
        let source = ModelId::from_str("mistral/mistral-large").unwrap();
        let catalog = offered(&["mistralai/mistral-large"]);
        let resolved = resolve_upstream_model(&source, &catalog).unwrap();
        assert_eq!(resolved.to_string(), "mistralai/mistral-large");
    }

    #[test]
    fn named_provider_passthrough_uses_gateway_id() {
        let source = ModelId::from_str("deepseek/deepseek-chat").unwrap();
        let catalog = offered(&["deepseek/deepseek-chat"]);
        let resolved = resolve_upstream_model(&source, &catalog).unwrap();
        assert_eq!(resolved.to_string(), "deepseek/deepseek-chat");
    }

    #[test]
    fn returns_none_when_slug_not_in_catalog() {
        let source = ModelId::from_str("totally-unknown/some-model").unwrap();
        let catalog = offered(&["openai/gpt-4o-mini"]);
        assert!(resolve_upstream_model(&source, &catalog).is_none());
    }

    #[test]
    fn preserves_free_suffix() {
        let source = ModelId::from_str("qwen/qwen3-coder:free").unwrap();
        let catalog = offered(&["qwen/qwen3-coder:free"]);
        let resolved = resolve_upstream_model(&source, &catalog).unwrap();
        assert_eq!(resolved.to_string(), "qwen/qwen3-coder:free");
    }
}
