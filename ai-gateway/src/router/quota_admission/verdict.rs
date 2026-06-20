use std::time::Duration;

use chrono::{DateTime, Utc};
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BlockedReason {
    None,
    Circuit,
    Rpm,
    Tpm,
    MinInterval,
    Rpd,
    Tpd,
    ModelCooldown,
    SlotCooldown,
    UpstreamReconcile,
}

#[derive(Debug, Clone)]
pub struct AdmissionVerdict {
    pub feasible: bool,
    pub next_wait: Duration,
    pub blocked_reason: BlockedReason,
    pub next_available_at: Option<DateTime<Utc>>,
}

impl AdmissionVerdict {
    #[must_use]
    pub fn from_blocking(
        next_wait: Duration,
        blocked_reason: BlockedReason,
    ) -> Self {
        let feasible =
            next_wait.is_zero() && blocked_reason == BlockedReason::None;
        let next_available_at = feasible.then(chrono::Utc::now).or_else(|| {
            Utc::now()
                .checked_add_signed(chrono::Duration::from_std(next_wait).ok()?)
        });
        Self {
            feasible,
            next_wait,
            blocked_reason,
            next_available_at,
        }
    }

    #[must_use]
    pub fn headroom_score(&self) -> f64 {
        if self.feasible { 1.0 } else { 0.0 }
    }
}
