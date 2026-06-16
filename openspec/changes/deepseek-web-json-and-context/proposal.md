## Why

`deepseek-web` shipped in beta.14 as a minimal browser-session provider: flat
prompt concatenation, no `response_format` support, and no multi-turn context
upload. Operators hit two production gaps immediately:

1. **Structured output** — autodefault rejects `json_schema` requests when
   `deepseek-web` is the only credential (`supports_json_schema: false`), and
   routed requests fail with `Converter not present` because no mapper is
   registered for the provider.
2. **Long payloads** — large dossiers are silently dropped or flattened via a
   naive `history_window`; unlike `chatgpt-web`, there is no token-budget
   chunking with context-upload turns.

`chatgpt-web` already solves both via `web-message-budget`, shared schema
instructions, executor multi-turn loops, and router integration. DeepSeek Web
must reach the same industrial bar without re-inventing the protocol.

Release target: **`0.3.0-beta.19`** (`Cargo.toml` is already **`0.3.0-beta.18`**
from `credential-secrets-local`; this change ships next).

## What Changes

- Add **strict / non-strict JSON schema** support for `deepseek-web`
  (`response_format.type = json_schema`), including prompt injection, response
  validation, bounded retries, autodefault eligibility, and mapper registration.
- Add **token-budget context chunking** for `deepseek-web` using the same
  upload-part protocol as `chatgpt-web` (no silent truncation; schema only on
  final turn).
- Reuse or extract **shared web structured-output primitives** so ChatGPT and
  DeepSeek do not fork schema parsing/validation logic.
- Update **CHANGELOG** with **`[0.3.0-beta.19]`** section and bump **`Cargo.toml`**.
- Replace the naive `messages_to_prompt(..., history_window)` path with
  `plan_web_chunks`-driven execution that keeps one `chat_session_id` for all
  upload turns in a single gateway request.

## Capabilities

### New Capabilities

- `deepseek-web-structured-output`: JSON schema / strict structured output for
  DeepSeek Web, router + autodefault integration, validation and retries.
- `deepseek-web-context-chunking`: Long-context handling via multi-turn context
  upload (ChatGPT-parity chunk plan), session reuse, pacing semantics.

### Modified Capabilities

- `deepseek-web-provider`: Extend base provider requirements with structured
  output and context chunking; bump documented release to beta.19.

## Impact

- **Crates:** `crates/deepseek-web`, `crates/web-message-budget` (constants /
  optional DeepSeek profile), possible new `crates/web-structured-output` or
  shared module extracted from `chatgpt-web`.
- **Gateway:** `ai-gateway/src/dispatcher/deepseek_web.rs`,
  `middleware/mapper/registry.rs`, `router/capability/providers.rs`,
  `router/budget_aware/structured_output.rs`.
- **Config / docs:** `provider-limits.yaml`, `docs/deepseek-web.md`,
  `docs/routing.md`.
- **Ops:** Large requests incur **multiple PoW + completion cycles** per gateway
  call; effective throughput drops — documented, not hidden.
