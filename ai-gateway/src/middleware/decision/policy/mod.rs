//! Per-key decision policy resolution and in-memory policy cache.

mod key_policy;
mod namespace;
mod store;
mod tier;

#[cfg(test)]
mod tests;

pub use key_policy::KeyPolicy;
pub use store::{MemoryPolicyStore, PolicyStore};
pub use tier::Tier;
