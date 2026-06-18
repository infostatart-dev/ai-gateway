use serde::{Deserialize, Serialize};

/// Client/upstream capability band for intent routing.
#[derive(
    Debug,
    Clone,
    Copy,
    Default,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Serialize,
    Deserialize,
)]
#[serde(rename_all = "kebab-case")]
pub enum IntentTier {
    Fast,
    #[serde(rename = "fast-thinking")]
    FastThinking,
    #[default]
    Standard,
    Deep,
}

impl IntentTier {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Fast => "fast",
            Self::FastThinking => "fast-thinking",
            Self::Standard => "standard",
            Self::Deep => "deep",
        }
    }
}

/// Derive upstream intent tier from provider catalog model slug.
#[must_use]
pub fn default_upstream_intent_tier(model_name: &str) -> IntentTier {
    let name = model_name.to_ascii_lowercase();

    if name.contains("o1")
        || name.contains("o3")
        || name.contains("o4")
        || name.contains("reasoner")
        || name.contains("thinking")
        || name.contains("opus")
        || name.contains("sonnet")
        || name.contains("70b")
        || name.contains("405b")
        || name.contains("r1")
        || name.contains("deepseek-r")
    {
        return IntentTier::Deep;
    }

    if name.contains("scout")
        || name.contains("gpt-oss")
        || name.contains("nemotron")
        || name.contains("magistral")
        || name.contains("gpt-4o-mini")
        || name.contains("gpt-4.1")
    {
        return IntentTier::FastThinking;
    }

    if name.contains("flash")
        || name.contains("lite")
        || name.contains("instant")
        || name.contains("8b-instant")
        || name.contains("3b-instruct")
        || name.ends_with(":free")
    {
        return IntentTier::Fast;
    }

    if name.contains("gpt-5") {
        return IntentTier::Deep;
    }

    IntentTier::Standard
}

/// Higher score = closer match to the client's preferred tier.
#[must_use]
pub fn intent_proximity_score(
    preferred: IntentTier,
    candidate: IntentTier,
) -> u16 {
    let diff = preferred.abs_distance(candidate);
    16_u16.saturating_sub(diff.saturating_mul(4))
}

impl IntentTier {
    #[must_use]
    const fn rank(self) -> u8 {
        match self {
            Self::Fast => 0,
            Self::FastThinking => 1,
            Self::Standard => 2,
            Self::Deep => 3,
        }
    }

    #[must_use]
    fn abs_distance(self, other: Self) -> u16 {
        let left = self.rank();
        let right = other.rank();
        if left >= right {
            (left - right).into()
        } else {
            (right - left).into()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upstream_tier_tags() {
        assert_eq!(
            default_upstream_intent_tier(
                "meta-llama/llama-4-scout-17b-16e-instruct"
            ),
            IntentTier::FastThinking
        );
        assert_eq!(
            default_upstream_intent_tier("gemini-2.0-flash"),
            IntentTier::Fast
        );
        assert_eq!(
            default_upstream_intent_tier("claude-sonnet-4-0"),
            IntentTier::Deep
        );
    }
}
