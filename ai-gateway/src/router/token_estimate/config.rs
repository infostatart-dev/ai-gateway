use serde_json::Value;

/// Default output reservation when a request omits `max_tokens`. Mirrors the
/// 4000-token output `OpenRouter` reserves when computing context overflow.
pub const DEFAULT_OUTPUT_TOKENS: u32 = 4_000;

/// Default safety margin (percent) shaved off a candidate's effective window to
/// absorb tokenizer error and provider accounting differences.
pub const DEFAULT_SAFETY_MARGIN_PCT: u8 = 5;

/// Tunable budget knobs for payload-aware routing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PayloadBudgetConfig {
    /// Output tokens reserved when the request does not specify `max_tokens`.
    pub default_output_tokens: u32,
    /// Percent of the effective window kept as headroom (0..=100).
    pub safety_margin_pct: u8,
}

impl Default for PayloadBudgetConfig {
    fn default() -> Self {
        Self {
            default_output_tokens: DEFAULT_OUTPUT_TOKENS,
            safety_margin_pct: DEFAULT_SAFETY_MARGIN_PCT,
        }
    }
}

impl PayloadBudgetConfig {
    /// Output tokens to reserve for `body`: its `max_tokens`
    /// (or `max_completion_tokens`) when present, else the configured default.
    #[must_use]
    pub fn reserved_output(self, body: &Value) -> u32 {
        ["max_tokens", "max_completion_tokens"]
            .iter()
            .find_map(|key| body.get(key).and_then(Value::as_u64))
            .and_then(|v| u32::try_from(v).ok())
            .unwrap_or(self.default_output_tokens)
    }

    /// Effective window after applying the safety margin.
    #[must_use]
    pub fn apply_margin(self, window: u32) -> u32 {
        let keep = 100u32.saturating_sub(u32::from(self.safety_margin_pct));
        (u64::from(window) * u64::from(keep) / 100)
            .try_into()
            .unwrap_or(u32::MAX)
    }
}
