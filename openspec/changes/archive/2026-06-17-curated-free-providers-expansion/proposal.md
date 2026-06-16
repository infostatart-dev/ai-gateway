## Why

Autodefault already spreads traffic across Gemini free slots, OpenRouter `:free`,
OpenCode, GitHub Models, Mistral, Cerebras, and Cloudflare — but several
high-volume documented free API tiers are missing. Groq is configured as paid
despite a no-card free developer tier. OpenRouter's free catalog is incomplete
relative to the live `:free` and `openrouter/free` offerings.

Adding Tier 1 API-key providers plus a Tier 2 OpenRouter expansion in one change
maximizes free inference headroom without browser-session providers or
rate-limit-only backends with no published token caps.

## What Changes

- Add eight new OpenAI-compatible free-tier providers with curated model catalogs,
  credential slots, conservative capability metadata, and provider limits:
  `longcat`, `doubao`, `ollama-cloud`, `inclusionai`, `sambanova`, `bluesminds`,
  `bazaarlink`, `cohere`.
- Reclassify existing `groq-default` from paid to `tier: free` /
  `cost-class: free`; align `provider-limits.yaml` tier key to `free`.
- Expand OpenRouter Tier 2: additional verified `:free` slugs, `openrouter/free`
  router model, and autodefault/model-mapping entries that prefer free targets.
- Update autodefault provider priority and `model-mapping.yaml` cost-first
  targets for nano/mini models.
- Add capability helpers, docs, secrets examples, and mock-backed integration
  tests for every new provider and the Groq/OpenRouter deltas.

Release target after implementation and tests: **`0.3.0-beta.20`** (from
`0.3.0-beta.19`).

## Capabilities

### New Capabilities

- `curated-free-providers-expansion`: Single specification covering all Tier 1
  API-key providers, Groq free reclassification, Tier 2 OpenRouter expansion,
  autodefault placement, provider limits, model mapping, capabilities, credentials,
  documentation, and tests.

### Modified Capabilities

- `autodefault-routing-priority`: Extend provider priority order and cost-first
  mapping rules to include the new free providers and reclassified Groq.

## Impact

- Provider catalog: `ai-gateway/config/embedded/providers.yaml`.
- Credential slots: `ai-gateway/config/embedded/credentials.yaml`,
  `dev/secrets.local.example.yaml`, `.env.template`.
- Autodefault order: `ai-gateway/src/config/read.rs`.
- Capability inference: `ai-gateway/src/router/capability/providers.rs`.
- Provider limits: `ai-gateway/config/embedded/provider-limits.yaml`.
- Model mapping: `ai-gateway/config/embedded/model-mapping.yaml`.
- Docs: `docs/providers.md`, `docs/credentials.md`.
- Tests: config parsing, autodefault gating, capability filtering, mock dispatch.
- Workspace version: **`0.3.0-beta.20`**.
