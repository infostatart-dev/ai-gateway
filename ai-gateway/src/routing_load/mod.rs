//! Routing load verification (testing feature): fixtures, assertions,
//! scenarios.

#![allow(clippy::must_use_candidate, clippy::implicit_hasher)]

pub mod assert_identity;
pub mod assert_stats;
pub mod payload;
pub mod responses;
pub mod router;
pub mod scenarios;

pub use router::RoutingLoadHarness;
