use crate::config::decision::DecisionTier;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tier {
    Free,
    Freemium,
    Paid,
}

impl From<DecisionTier> for Tier {
    fn from(value: DecisionTier) -> Self {
        match value {
            DecisionTier::Free => Self::Free,
            DecisionTier::Freemium => Self::Freemium,
            DecisionTier::Paid => Self::Paid,
        }
    }
}
