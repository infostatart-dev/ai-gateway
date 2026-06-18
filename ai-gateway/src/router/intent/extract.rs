use super::tier::IntentTier;
use crate::types::model_id::ModelId;

/// Resolved client intent from model name (payload filters are separate).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RoutingIntent {
    pub preferred_tier: IntentTier,
    pub floor_tier: IntentTier,
    pub escalation_ceiling: IntentTier,
}

impl Default for RoutingIntent {
    fn default() -> Self {
        Self {
            preferred_tier: IntentTier::Standard,
            floor_tier: IntentTier::Standard,
            escalation_ceiling: IntentTier::Deep,
        }
    }
}

impl RoutingIntent {
    #[must_use]
    pub const fn new(
        preferred_tier: IntentTier,
        floor_tier: IntentTier,
        escalation_ceiling: IntentTier,
    ) -> Self {
        Self {
            preferred_tier,
            floor_tier,
            escalation_ceiling,
        }
    }

    /// Plain chat widens the pool to fast-tier upstream; strict json keeps
    /// floor.
    #[must_use]
    pub fn effective_floor(
        self,
        requirements: &crate::router::capability::RequestRequirements,
    ) -> IntentTier {
        if requirements.json_schema_required {
            return self.floor_tier;
        }
        if self.floor_tier == IntentTier::FastThinking {
            IntentTier::Fast
        } else {
            self.floor_tier
        }
    }
}

#[must_use]
pub fn extract_routing_intent(source_model: &ModelId) -> RoutingIntent {
    client_intent_from_model_name(&source_model.to_string())
}

#[must_use]
pub fn extract_routing_intent_from_name(model_name: &str) -> RoutingIntent {
    client_intent_from_model_name(model_name)
}

fn client_intent_from_model_name(model_name: &str) -> RoutingIntent {
    let name = normalize_model_name(model_name);

    if is_fast_thinking_gpt5(&name) {
        return RoutingIntent::new(
            IntentTier::FastThinking,
            IntentTier::FastThinking,
            IntentTier::Deep,
        );
    }

    if is_deep_client_model(&name) {
        return RoutingIntent::new(
            IntentTier::Deep,
            IntentTier::Deep,
            IntentTier::Deep,
        );
    }

    if name.contains("nano")
        || name.contains("flash")
        || name.contains("lite")
        || name.contains("instant")
        || name.contains("8b-instant")
    {
        return RoutingIntent::new(
            IntentTier::Fast,
            IntentTier::Fast,
            IntentTier::Deep,
        );
    }

    if name.contains("mini") || name.contains("small") || name.contains("haiku")
    {
        return RoutingIntent::new(
            IntentTier::FastThinking,
            IntentTier::FastThinking,
            IntentTier::Deep,
        );
    }

    RoutingIntent::default()
}

fn normalize_model_name(model_name: &str) -> String {
    let name = model_name.to_ascii_lowercase();
    name.rsplit('/').next().unwrap_or(&name).to_string()
}

fn is_fast_thinking_gpt5(name: &str) -> bool {
    name.contains("gpt-5-nano")
        || name.contains("gpt-5.4-nano")
        || name.contains("gpt-5-mini")
        || name.contains("gpt-5.4-mini")
}

fn is_deep_client_model(name: &str) -> bool {
    if is_fast_thinking_gpt5(name) {
        return false;
    }
    name.contains("o1")
        || name.contains("o3")
        || name.contains("o4")
        || name.contains("reasoner")
        || name.contains("thinking")
        || name.contains("opus")
        || name.contains("gpt-5")
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::*;
    use crate::types::model_id::ModelId;

    #[test]
    fn mini_and_nano_share_fast_thinking_intent() {
        let mini = extract_routing_intent_from_name("openai/gpt-5-mini");
        let nano = extract_routing_intent_from_name("openai/gpt-5-nano");
        assert_eq!(mini, nano);
        assert_eq!(mini.preferred_tier, IntentTier::FastThinking);
    }

    #[test]
    fn plain_gpt5_is_deep() {
        let intent = extract_routing_intent_from_name("openai/gpt-5");
        assert_eq!(intent.preferred_tier, IntentTier::Deep);
        assert_eq!(intent.floor_tier, IntentTier::Deep);
    }

    #[test]
    fn nano_is_not_deep() {
        let intent = extract_routing_intent_from_name("openai/gpt-5.4-nano");
        assert_ne!(intent.preferred_tier, IntentTier::Deep);
    }

    #[test]
    fn model_id_extraction() {
        let model = ModelId::from_str("openai/gpt-5-mini").expect("model id");
        assert_eq!(
            extract_routing_intent(&model).preferred_tier,
            IntentTier::FastThinking
        );
    }
}
