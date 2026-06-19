//! Routing load harness — test-only helpers (`feature = "testing"`).

pub mod assert_identity;
pub mod assert_stats;
pub mod headers;
pub mod pacing_harness;
pub mod payload;
pub mod responses;
pub mod router;
pub mod run;

pub use headers::{agent_header, caller_parts, work_unit_header};
pub use router::{RoutingLoadHarness, prepare_harness_test};
pub use run::{PlannedResponse, run_planned_failover};
