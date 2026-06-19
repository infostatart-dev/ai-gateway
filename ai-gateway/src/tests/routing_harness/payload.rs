use bytes::Bytes;
use serde_json::json;

/// Extra user-message chars that push estimates above groq free TPM (12k →
/// ~11.4k effective window after margin) while staying inside Gemini free
/// slots.
pub const GROQ_FILTER_EXTRA_CHARS: usize = 60_000;

#[must_use]
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
#[must_use]
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

#[must_use]
pub fn default_fat_body() -> Bytes {
    fat_json_schema_body(12_000)
}

/// Minimal strict-json body for autodefault intent routing load tests.
#[must_use]
pub fn nano_json_strict_body() -> Bytes {
    Bytes::from(
        json!({
            "model": "openai/gpt-5-nano",
            "stream": false,
            "response_format": {
                "type": "json_schema",
                "json_schema": {
                    "name": "intent_load",
                    "strict": true,
                    "schema": {
                        "type": "object",
                        "properties": { "ok": {"type": "boolean"} },
                        "required": ["ok"],
                        "additionalProperties": false
                    }
                }
            },
            "messages": [{ "role": "user", "content": "ping" }]
        })
        .to_string(),
    )
}
