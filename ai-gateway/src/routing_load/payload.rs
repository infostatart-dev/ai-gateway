use bytes::Bytes;
use serde_json::json;

/// Extra user-message chars that push estimates above groq free TPM (12k →
/// ~11.4k effective window after margin) while staying inside Gemini free
/// slots.
pub const GROQ_FILTER_EXTRA_CHARS: usize = 60_000;

pub fn fat_json_schema_body(extra_chars: usize) -> Bytes {
    let filler = "x".repeat(extra_chars);
    Bytes::from(
        json!({
            "model": "openai/gpt-4o-mini",
            "stream": false,
            "response_format": {
                "type": "json_schema",
                "json_schema": {
                    "name": "routing_load_dossier",
                    "strict": true,
                    "schema": {
                        "type": "object",
                        "properties": {
                            "value": {"type": "string"},
                            "details": {"type": "string"}
                        },
                        "required": ["value", "details"],
                        "additionalProperties": false
                    }
                }
            },
            "messages": [{
                "role": "user",
                "content": format!("routing load dossier {filler}")
            }]
        })
        .to_string(),
    )
}

/// Large chat payload without `response_format` — same token footprint for
/// routing tests that go through the HTTP harness (stub bodies are not
/// schema-valid).
pub fn large_chat_body(extra_chars: usize) -> Bytes {
    let filler = "x".repeat(extra_chars);
    Bytes::from(
        json!({
            "model": "openai/gpt-4o-mini",
            "messages": [{
                "role": "user",
                "content": format!("routing load dossier {filler}")
            }]
        })
        .to_string(),
    )
}

pub fn default_fat_body() -> Bytes {
    fat_json_schema_body(12_000)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::router::token_estimate::{
        PayloadBudgetConfig, estimate_from_value,
    };

    #[test]
    fn default_fat_body_exceeds_groq_free_tpm_cap() {
        let body = fat_json_schema_body(GROQ_FILTER_EXTRA_CHARS);
        let parsed: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let estimate =
            estimate_from_value(&parsed, PayloadBudgetConfig::default())
                .expect("estimate");
        assert!(
            estimate.total() > 11_400,
            "expected payload above groq TPM window, got {}",
            estimate.total()
        );
    }

    #[test]
    fn large_chat_body_exceeds_groq_free_tpm_cap() {
        let body = large_chat_body(GROQ_FILTER_EXTRA_CHARS);
        let parsed: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let estimate =
            estimate_from_value(&parsed, PayloadBudgetConfig::default())
                .expect("estimate");
        assert!(
            estimate.total() > 11_400,
            "expected payload above groq TPM window, got {}",
            estimate.total()
        );
    }
}
