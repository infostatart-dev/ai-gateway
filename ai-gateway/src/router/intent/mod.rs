//! Client model name → routing intent (tier bands, stability floor/ceiling).

mod extract;
mod tier;

pub use extract::{
    RoutingIntent, extract_routing_intent, extract_routing_intent_from_name,
};
pub use tier::{
    IntentTier, default_upstream_intent_tier, intent_proximity_score,
};

/// Whether failover selected a candidate above the preferred intent band.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionPhase {
    Preferred,
    Escalated,
}

impl SelectionPhase {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Preferred => "preferred",
            Self::Escalated => "escalated",
        }
    }
}
