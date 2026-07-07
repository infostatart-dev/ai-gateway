use http::{HeaderMap, HeaderName, HeaderValue};
use serde::Serialize;

pub const SMART_STATUS_HEADER: HeaderName =
    HeaderName::from_static("x-smart-status");
pub const SMART_WARNING_HEADER: HeaderName =
    HeaderName::from_static("x-smart-warning");
pub const STABLE_BINDING_STATUS: &str =
    "Stable binding Model - you use stability bindings";
pub const UNSTABLE_GPT55_WARNING: &str =
    "Unstable binding Model - openai/gpt-5.5 is not a stability binding; use \
     openai/gpt-5.5-mini or openai/gpt-5.5-nano";

const STABLE_MODELS: &[DeclaredModelSpec] = &[
    DeclaredModelSpec {
        id: "openai/gpt-5.5-nano",
        stability: ModelStability::Stable,
        canonical_binding: Some("openai/gpt-5.4-nano"),
        warning: None,
    },
    DeclaredModelSpec {
        id: "openai/gpt-5.5-mini",
        stability: ModelStability::Stable,
        canonical_binding: Some("openai/gpt-5.4-mini"),
        warning: None,
    },
];

const UNSTABLE_MODELS: &[DeclaredModelSpec] = &[DeclaredModelSpec {
    id: "openai/gpt-5.5",
    stability: ModelStability::Unstable,
    canonical_binding: None,
    warning: Some(UNSTABLE_GPT55_WARNING),
}];

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "lowercase")]
enum ModelStability {
    Stable,
    Unstable,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct DeclaredModelSpec {
    pub id: &'static str,
    stability: ModelStability,
    #[serde(skip_serializing_if = "Option::is_none")]
    canonical_binding: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    warning: Option<&'static str>,
}

#[derive(Debug, Serialize)]
pub struct DeclaredModelCatalog {
    pub stable: &'static [DeclaredModelSpec],
    pub unstable: &'static [DeclaredModelSpec],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeclaredModelStatus {
    Stable,
    UnstableGpt55,
}

#[must_use]
pub const fn catalog() -> DeclaredModelCatalog {
    DeclaredModelCatalog {
        stable: STABLE_MODELS,
        unstable: UNSTABLE_MODELS,
    }
}

#[must_use]
pub fn classify(model: &str) -> Option<DeclaredModelStatus> {
    match normalized_slug(model).as_str() {
        "gpt-5.5-nano" | "gpt-5.5-mini" => Some(DeclaredModelStatus::Stable),
        "gpt-5.5" => Some(DeclaredModelStatus::UnstableGpt55),
        _ => None,
    }
}

#[must_use]
pub fn is_declared_gateway_binding(model: &str) -> bool {
    let requested = model.trim();
    catalog()
        .stable
        .iter()
        .chain(catalog().unstable.iter())
        .any(|spec| spec.id == requested)
}

#[must_use]
pub fn canonical_mapping_slug(model_name: &str) -> Option<&'static str> {
    match normalized_slug(model_name).as_str() {
        "gpt-5.5-nano" => Some("gpt-5.4-nano"),
        "gpt-5.5-mini" => Some("gpt-5.4-mini"),
        _ => None,
    }
}

pub fn apply_smart_headers(headers: &mut HeaderMap, source_model: &str) {
    match classify(source_model) {
        Some(DeclaredModelStatus::Stable) => {
            headers.insert(
                SMART_STATUS_HEADER,
                HeaderValue::from_static(STABLE_BINDING_STATUS),
            );
        }
        Some(DeclaredModelStatus::UnstableGpt55) => {
            headers.insert(
                SMART_WARNING_HEADER,
                HeaderValue::from_static(UNSTABLE_GPT55_WARNING),
            );
        }
        None => {}
    }
}

fn normalized_slug(model: &str) -> String {
    let slug = model
        .trim()
        .to_ascii_lowercase()
        .rsplit('/')
        .next()
        .unwrap_or_default()
        .to_string();
    slug.strip_prefix("gpt5.")
        .map_or(slug.clone(), |rest| format!("gpt-5.{rest}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_declared_stable_bindings() {
        assert_eq!(
            classify("openai/gpt-5.5-nano"),
            Some(DeclaredModelStatus::Stable)
        );
        assert_eq!(classify("gpt5.5-mini"), Some(DeclaredModelStatus::Stable));
    }

    #[test]
    fn classifies_unstable_plain_gpt55() {
        assert_eq!(
            classify("openai/gpt-5.5"),
            Some(DeclaredModelStatus::UnstableGpt55)
        );
    }

    #[test]
    fn admission_uses_declared_catalog_ids_only() {
        assert!(is_declared_gateway_binding("openai/gpt-5.5-mini"));
        assert!(is_declared_gateway_binding("openai/gpt-5.5-nano"));
        assert!(is_declared_gateway_binding("openai/gpt-5.5"));

        assert!(!is_declared_gateway_binding("gpt-5.5-mini"));
        assert!(!is_declared_gateway_binding("glm-4.5-air:free"));
        assert!(!is_declared_gateway_binding("openrouter/openrouter/free"));
        assert!(!is_declared_gateway_binding("gemini/gemini-3.1-pro"));
    }

    #[test]
    fn maps_declared_stable_bindings_to_existing_gateway_lookup_aliases() {
        assert_eq!(
            canonical_mapping_slug("gpt-5.5-nano"),
            Some("gpt-5.4-nano")
        );
        assert_eq!(
            canonical_mapping_slug("openai/gpt-5.5-mini"),
            Some("gpt-5.4-mini")
        );
    }
}
