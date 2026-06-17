use std::collections::HashMap;

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct EmulatorConfig {
    #[serde(default = "default_address")]
    pub address: String,
    #[serde(default = "default_port")]
    pub port: u16,
    #[serde(default = "default_latency_ms")]
    pub default_latency_ms: u64,
    #[serde(default = "default_ms_per_token")]
    pub ms_per_token: f64,
    #[serde(default = "default_one")]
    pub latency_multiplier: f64,
    #[serde(default)]
    pub provider_latency_ms: HashMap<String, u64>,
}

impl Default for EmulatorConfig {
    fn default() -> Self {
        Self {
            address: default_address(),
            port: default_port(),
            default_latency_ms: default_latency_ms(),
            ms_per_token: default_ms_per_token(),
            latency_multiplier: default_one(),
            provider_latency_ms: realistic_provider_latencies(),
        }
    }
}

/// Realistic TTFB estimates (ms) based on observed provider behaviour.
/// Used as `base_ms` in `delay = base_ms + tokens * ms_per_token`.
fn realistic_provider_latencies() -> HashMap<String, u64> {
    [
        ("cerebras", 80u64),
        ("sambanova", 100),
        ("groq", 150),
        ("inclusionai", 180),
        ("bluesminds", 180),
        ("bazaarlink", 200),
        ("longcat", 200),
        ("cloudflare", 200),
        ("gemini", 220),
        ("ollama-cloud", 250),
        ("mistral", 300),
        ("cohere", 300),
        ("openrouter", 320),
        ("openai", 450),
        ("github-models", 420),
        ("doubao", 400),
        ("opencode", 430),
        ("anthropic", 580),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_string(), v))
    .collect()
}

fn default_address() -> String {
    "127.0.0.1".into()
}

fn default_port() -> u16 {
    5151
}

fn default_latency_ms() -> u64 {
    200
}

fn default_ms_per_token() -> f64 {
    0.02
}

fn default_one() -> f64 {
    1.0
}

impl EmulatorConfig {
    #[must_use]
    pub fn latency_for(&self, provider: &str, total_tokens: u32) -> u64 {
        let base = self
            .provider_latency_ms
            .get(provider)
            .copied()
            .unwrap_or(self.default_latency_ms);
        let token_ms =
            (f64::from(total_tokens) * self.ms_per_token).round() as u64;
        scale_latency_ms(base.saturating_add(token_ms), self.latency_multiplier)
    }
}

fn scale_latency_ms(base_ms: u64, multiplier: f64) -> u64 {
    if !multiplier.is_finite() || multiplier <= 0.0 {
        return 0;
    }
    if (multiplier - 1.0).abs() < f64::EPSILON {
        return base_ms;
    }
    #[allow(
        clippy::cast_precision_loss,
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss
    )]
    {
        let scaled = (base_ms as f64) * multiplier;
        scaled.round().max(0.0) as u64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fat_payload_has_higher_latency_than_hello() {
        let config = EmulatorConfig::default();
        let hello = config.latency_for("groq", 10);
        let fat = config.latency_for("groq", 20_000);
        assert!(fat > hello);
    }
}
