use crate::config::decision::DecisionTier;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Tier {
    Free,
    Paid,
}

impl From<DecisionTier> for Tier {
    fn from(value: DecisionTier) -> Self {
        match value {
            DecisionTier::Free => Self::Free,
            DecisionTier::Paid => Self::Paid,
        }
    }
}
