//! Pre-flight payload token estimation for payload-aware routing.
//!
//! Estimates the input token count of a chat request (messages + tools +
//! `response_format` `json_schema`) and the reserved output budget so the
//! router can drop candidates whose context window or per-minute token cap
//! cannot fit the request before any upstream call is made.

mod config;
mod encode;
mod extract;

#[cfg(test)]
mod tests;

pub use config::PayloadBudgetConfig;
use serde_json::Value;

/// Estimated token footprint of a single chat request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PayloadEstimate {
    /// Tokens attributed to the serialized prompt (incl. the `json_schema`).
    pub input_tokens: u32,
    /// Output tokens reserved from `max_tokens` (or the configured default).
    pub reserved_output: u32,
}

impl PayloadEstimate {
    /// Total tokens that must fit inside a candidate's effective window.
    #[must_use]
    pub fn total(self) -> u32 {
        self.input_tokens.saturating_add(self.reserved_output)
    }
}

/// Estimate the token footprint from an already-parsed request value. Returns
/// `None` for non-object bodies (fail-open).
#[must_use]
pub fn estimate_from_value(
    value: &Value,
    config: PayloadBudgetConfig,
) -> Option<PayloadEstimate> {
    if !value.is_object() {
        return None;
    }
    let input_tokens = encode::count_tokens(&extract::billable_text(value));
    Some(PayloadEstimate {
        input_tokens,
        reserved_output: config.reserved_output(value),
    })
}
