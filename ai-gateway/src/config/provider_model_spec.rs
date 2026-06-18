//! Parsed provider model entries: upstream wire slug vs catalog limits key.

use indexmap::IndexMap;
use serde::Deserialize;

use crate::types::{model_id::ModelId, provider::InferenceProvider};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderModelEntry {
    pub upstream_slug: String,
    pub catalog_key: String,
}

pub type ModelCatalogMaps = (
    IndexMap<ModelId, ProviderModelEntry>,
    IndexMap<ModelId, String>,
);

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum RawModelSpec {
    Slug(String),
    Entry {
        upstream: String,
        #[serde(default)]
        catalog: Option<String>,
    },
}

impl RawModelSpec {
    #[must_use]
    pub fn upstream_slug(&self) -> &str {
        match self {
            Self::Slug(s) => s.as_str(),
            Self::Entry { upstream, .. } => upstream.as_str(),
        }
    }

    #[must_use]
    pub fn catalog_key(&self) -> String {
        match self {
            Self::Slug(s) => s.clone(),
            Self::Entry { upstream, catalog } => {
                catalog.clone().unwrap_or_else(|| upstream.clone())
            }
        }
    }
}

pub fn build_model_catalog_keys(
    provider: &InferenceProvider,
    specs: &[RawModelSpec],
) -> Result<ModelCatalogMaps, String> {
    let mut entries = IndexMap::new();
    let mut catalog_keys = IndexMap::new();
    for spec in specs {
        let upstream = spec.upstream_slug();
        let model_id =
            ModelId::from_str_and_provider(provider.clone(), upstream)
                .map_err(|e| {
                    format!("invalid model '{upstream}' for {provider}: {e}")
                })?;
        let catalog_key = spec.catalog_key();
        catalog_keys.insert(model_id.clone(), catalog_key.clone());
        entries.insert(
            model_id,
            ProviderModelEntry {
                upstream_slug: upstream.to_string(),
                catalog_key,
            },
        );
    }
    Ok((entries, catalog_keys))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn structured_entry_splits_upstream_and_catalog() {
        let spec = RawModelSpec::Entry {
            upstream: "gemini-3-flash-preview".into(),
            catalog: Some("gemini-3-flash".into()),
        };
        assert_eq!(spec.upstream_slug(), "gemini-3-flash-preview");
        assert_eq!(spec.catalog_key(), "gemini-3-flash");
    }

    #[test]
    fn bare_slug_uses_same_catalog_key() {
        let spec = RawModelSpec::Slug("gemini-3.5-flash".into());
        assert_eq!(spec.catalog_key(), "gemini-3.5-flash");
    }
}
