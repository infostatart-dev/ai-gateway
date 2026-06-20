//! Hierarchical quota admission on [`PacingScope`] (L0 tier → L1 account → L2
//! model).

mod evaluate;
mod pacing;
mod verdict;

pub use evaluate::evaluate_candidate;
pub use pacing::{PacingAdmissionScope, evaluate_pacing_admission};
pub use verdict::{AdmissionVerdict, BlockedReason};
