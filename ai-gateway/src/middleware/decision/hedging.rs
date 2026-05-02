use std::time::Duration;
use tokio::time::Instant;

/// Orchestrates a hedging strategy where a cheap request is fired first,
/// and if a meaningful token isn't received within the TTFT threshold,
/// a secondary (expensive) request is fired.
pub struct HedgingOrchestrator {
    ttft_threshold: Duration,
}

impl HedgingOrchestrator {
    pub fn new(ttft_threshold: Duration) -> Self {
        Self { ttft_threshold }
    }

    /// Wait for the TTFT threshold. If the primary stream does not produce
    /// a meaningful token (one with actual text content, not just metadata)
    /// within this duration, this method will return, allowing the caller
    /// to trigger the hedge request.
    ///
    /// In actual usage, this requires deep integration with the stream polling
    /// mechanism to inspect chunks. For the decision engine layer, we define
    /// the config and policy.
    pub async fn wait_for_hedge_trigger(&self, start: Instant) {
        tokio::time::sleep_until(start + self.ttft_threshold).await;
    }
}

/// Helper function for checking if an OpenAI-compatible SSE chunk contains a meaningful token.
pub fn is_meaningful_token(chunk: &str) -> bool {
    // Ignore empty lines and SSE comments
    if chunk.trim().is_empty() || chunk.starts_with(':') {
        return false;
    }

    // Typical OpenAI delta format:
    // data: {"choices":[{"delta":{"content":"Hello"}}]}
    if let Some(data) = chunk.strip_prefix("data: ") {
        if data.trim() == "[DONE]" {
            return false;
        }

        if let Ok(json) = serde_json::from_str::<serde_json::Value>(data) {
            if let Some(choices) = json.get("choices").and_then(|v| v.as_array()) {
                for choice in choices {
                    if let Some(content) = choice.pointer("/delta/content").and_then(|v| v.as_str()) {
                        if !content.trim().is_empty() {
                            return true;
                        }
                    }
                }
            }
        }
    }

    false
}
