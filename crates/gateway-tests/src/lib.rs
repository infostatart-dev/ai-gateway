//! Workspace test utilities — declarative upstream mocks, no FIFO script
//! queues.
//!
//! Production code MUST NOT depend on this crate. Use as `dev-dependency` or
//! via `ai-gateway` feature `testing`.

pub mod upstream;

pub use upstream::{
    HopTarget, ResponseFactory, UpstreamMockScript, clear_upstream_mocks,
    install_upstream_mock, pop_upstream_response,
};
