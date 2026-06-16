use serde::{Deserialize, Serialize};

use crate::types::provider::InferenceProvider;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CostClass {
    Free,
    Paid,
    PaidBrowser,
}

impl CostClass {
    #[must_use]
    pub const fn rank_base(self) -> u16 {
        match self {
            Self::Free => 0,
            Self::Paid => 200,
            Self::PaidBrowser => 300,
        }
    }
}

#[must_use]
pub fn derive_cost_class(
    explicit: Option<CostClass>,
    provider: &InferenceProvider,
    tier: &str,
) -> CostClass {
    if let Some(class) = explicit {
        return class;
    }
    if crate::config::chatgpt_web::is_chatgpt_web(provider) {
        return CostClass::PaidBrowser;
    }
    if crate::config::deepseek_web::is_deepseek_web(provider) {
        return CostClass::Free;
    }
    match tier {
        "free" => CostClass::Free,
        _ => CostClass::Paid,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn free_tier_api_slot_resolves_free() {
        let openrouter = InferenceProvider::OpenRouter;
        assert_eq!(
            derive_cost_class(None, &openrouter, "free"),
            CostClass::Free
        );
    }

    #[test]
    fn tier_3_slot_resolves_paid() {
        let gemini = InferenceProvider::GoogleGemini;
        assert_eq!(derive_cost_class(None, &gemini, "tier-3"), CostClass::Paid);
    }

    #[test]
    fn chatgpt_web_resolves_paid_browser() {
        let chatgpt = InferenceProvider::Named("chatgpt-web".into());
        assert_eq!(
            derive_cost_class(None, &chatgpt, "session"),
            CostClass::PaidBrowser
        );
    }

    #[test]
    fn deepseek_web_resolves_free() {
        let deepseek = InferenceProvider::Named("deepseek-web".into());
        assert_eq!(
            derive_cost_class(None, &deepseek, "session"),
            CostClass::Free
        );
    }

    #[test]
    fn explicit_cost_class_overrides_tier() {
        let gemini = InferenceProvider::GoogleGemini;
        assert_eq!(
            derive_cost_class(Some(CostClass::Free), &gemini, "tier-3"),
            CostClass::Free
        );
    }
}
