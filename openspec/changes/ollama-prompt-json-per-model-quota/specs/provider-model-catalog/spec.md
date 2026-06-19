## ADDED Requirements

### Requirement: json-schema-delivery catalog field

Each model capability entry in embedded provider catalog SHALL support an optional
`json-schema-delivery` field with one of:

- `native` — upstream API accepts and enforces `response_format` / provider-native structured output
- `prompt` — structured output achieved only via gateway system-prompt injection; upstream native field unsupported or ignored
- `none` — default; model not used for json_schema routing

When omitted, the gateway SHALL treat delivery as `none`.

#### Scenario: Ollama Cloud free gpt-oss declares prompt delivery

- **WHEN** embedded catalog loads `ollama-cloud/gpt-oss:120b`
- **THEN** `json-schema-delivery: prompt` is present
- **AND** `supports-json-schema: false` native flag does not exclude the model from prompt-json routing

#### Scenario: OpenRouter free slug declares native delivery

- **WHEN** embedded catalog loads `openrouter/openai/gpt-oss-120b:free`
- **THEN** `json-schema-delivery: native` is present or inferred from verified native API support

---

### Requirement: Ollama Cloud catalog verification metadata

The `ollama-cloud` provider block SHALL record `last_verified_at` and notes that
native structured outputs are unsupported on Cloud while prompt-json injection
was verified on listed free slugs.

#### Scenario: Verification date present after refresh

- **WHEN** embedded Ollama Cloud catalog is loaded after this change
- **THEN** `last_verified_at` is `2026-06-19` or later
- **AND** notes reference official Cloud structured-output limitation and curl verification method
