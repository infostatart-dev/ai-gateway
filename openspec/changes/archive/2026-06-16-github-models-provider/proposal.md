## Why

GitHub Models offers useful free-tier access to GPT, o-series, DeepSeek, Llama,
Grok, Mistral, Cohere, and Phi models through a standard GitHub PAT and an
OpenAI-compatible chat-completions API. Adding `github-models` gives the gateway
another high-quality free fallback without browser sessions or custom auth flows.

## What Changes

- Add `github-models` as a first-class provider with GitHub-specific static
  headers (`X-GitHub-Api-Version`, `Accept`).
- Support PAT credentials via `github-models-default` /
  `AI_GATEWAY_CREDENTIAL_GITHUB_MODELS_DEFAULT` (`models:read` scope).
- Register 12 curated chat models plus 2 embedding IDs (catalog only in v1).
- Add per-model context windows and conservative capability metadata.
- Gate autodefault inclusion on credential presence; priority after `openrouter`.
- Add provider limits, docs, and mock-backed tests.

Release target after implementation and tests: **`0.3.0-beta.15`** (from
`0.3.0-beta.14`).

## Capabilities

### New Capabilities

- `github-models-provider`: GitHub Models provider integration, credentials,
  model catalog, routing, autodefault gating, and tests.

### Modified Capabilities

- None.

## Impact

- Provider catalog: `ai-gateway/config/embedded/providers.yaml`.
- Provider config schema / dispatcher: static `request-headers` support for
  `github-models`.
- Credential slots: `ai-gateway/config/embedded/credentials.yaml`, `.env.template`.
- Autodefault order: `ai-gateway/src/config/read.rs`.
- Provider limits: `ai-gateway/config/embedded/provider-limits.yaml`.
- Docs: `docs/providers.md` (GitHub Models section).
- Workspace version: **`0.3.0-beta.15`**.
