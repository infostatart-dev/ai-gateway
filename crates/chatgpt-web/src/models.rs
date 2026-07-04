use std::{collections::HashMap, sync::LazyLock};

static MODEL_MAP: LazyLock<HashMap<&'static str, &'static str>> =
    LazyLock::new(|| {
        HashMap::from([
            ("gpt-5-5-pro", "gpt-5-5-pro"),
            ("gpt-5-5-pro-extended", "gpt-5-5-pro"),
            ("gpt-5-5-thinking", "gpt-5-5-thinking"),
            ("gpt-5-5", "gpt-5-5"),
            ("gpt-5-4-pro", "gpt-5-4-pro"),
            ("gpt-5-4-thinking", "gpt-5-4-thinking"),
            ("gpt-5-4-t-mini", "gpt-5-4-t-mini"),
            ("gpt-5-3", "gpt-5-3"),
            ("gpt-5-3-mini", "gpt-5-3-mini"),
            ("gpt-5.5-pro", "gpt-5-5-pro"),
            ("gpt-5.5-pro-extended", "gpt-5-5-pro"),
            ("gpt-5.5-thinking", "gpt-5-5-thinking"),
            ("gpt-5.5", "gpt-5-5"),
            ("gpt-5.4-pro", "gpt-5-4-pro"),
            ("gpt-5.4-thinking", "gpt-5-4-thinking"),
            ("gpt-5.4-thinking-mini", "gpt-5-4-t-mini"),
            ("gpt-5.3-instant", "gpt-5-3-instant"),
            ("gpt-5.3", "gpt-5-3"),
            ("gpt-5.3-mini", "gpt-5-3-mini"),
            ("gpt-5.5-instant", "gpt-5-5-instant"),
            ("gpt-5.2-instant", "gpt-5-2-instant"),
            ("gpt-5.2", "gpt-5-2"),
            ("gpt-5.2-thinking", "gpt-5-2-thinking"),
            ("gpt-5.1", "gpt-5-1"),
            ("gpt-5", "gpt-5"),
            ("gpt-5-mini", "gpt-5-mini"),
            ("o3", "o3"),
        ])
    });

pub fn map_model(model: &str) -> String {
    MODEL_MAP
        .get(model)
        .map(|s| (*s).to_string())
        .unwrap_or_else(|| model.to_string())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThinkingEffort {
    Standard,
    Extended,
}

impl ThinkingEffort {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Standard => "standard",
            Self::Extended => "extended",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedModel {
    pub slug: String,
    pub effort: Option<ThinkingEffort>,
}

#[must_use]
pub fn resolve_model(model: &str, body: &serde_json::Value) -> ResolvedModel {
    let slug = map_model(model);
    let forced = match model {
        "gpt-5-5-pro" | "gpt-5.5-pro" => Some(ThinkingEffort::Standard),
        "gpt-5-5-pro-extended" | "gpt-5.5-pro-extended" => {
            Some(ThinkingEffort::Extended)
        }
        _ => None,
    };
    let requested = body
        .get("reasoning_effort")
        .and_then(serde_json::Value::as_str)
        .or_else(|| {
            body.pointer("/reasoning/effort")
                .and_then(serde_json::Value::as_str)
        })
        .and_then(normalize_thinking_effort);
    ResolvedModel {
        slug,
        effort: forced.or(requested),
    }
}

#[must_use]
pub fn is_thinking_capable(model_id: &str, slug: &str) -> bool {
    model_id.contains("thinking")
        || model_id == "o3"
        || slug.contains("thinking")
        || slug == "gpt-5-4-t-mini"
        || slug == "o3"
}

fn normalize_thinking_effort(input: &str) -> Option<ThinkingEffort> {
    match input.trim().to_ascii_lowercase().as_str() {
        "extended" | "high" | "xhigh" => Some(ThinkingEffort::Extended),
        "standard" | "low" | "medium" | "minimal" => {
            Some(ThinkingEffort::Standard)
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_dot_to_dash() {
        assert_eq!(map_model("gpt-5-mini"), "gpt-5-mini");
        assert_eq!(map_model("gpt-5.3-instant"), "gpt-5-3-instant");
        assert_eq!(map_model("gpt-5.5-pro"), "gpt-5-5-pro");
        assert_eq!(map_model("gpt-5.5"), "gpt-5-5");
    }

    #[test]
    fn resolves_forced_pro_effort() {
        let resolved =
            resolve_model("gpt-5.5-pro-extended", &serde_json::json!({}));
        assert_eq!(resolved.slug, "gpt-5-5-pro");
        assert_eq!(resolved.effort, Some(ThinkingEffort::Extended));
    }

    #[test]
    fn resolves_openai_reasoning_effort() {
        let resolved = resolve_model(
            "gpt-5.4-thinking-mini",
            &serde_json::json!({"reasoning_effort":"high"}),
        );
        assert_eq!(resolved.slug, "gpt-5-4-t-mini");
        assert_eq!(resolved.effort, Some(ThinkingEffort::Extended));
        assert!(is_thinking_capable("gpt-5.4-thinking-mini", &resolved.slug));
    }
}
