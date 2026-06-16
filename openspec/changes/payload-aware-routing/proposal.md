## Why

On `autodefault` (`budget-aware-capability-after`, `free-up`) with fat Sales-QA
payloads (json_schema + 60k–128k input tokens), the router blindly dispatches
every request down the full candidate chain regardless of payload size or
per-credential quota state. Observed gateway logs (`llm-gateway:0.3.0-beta.12`,
~2h window) show ~35% of all upstream hops are guaranteed-dead: groq `413` (TPM 30000 vs
requested 60946), OpenRouter `400` (context length 131072 vs ~128k input + 4000
output), cloudflare `429` (daily neurons exhausted), and Gemini free keys burned
4-at-a-time on `RESOURCE_EXHAUSTED`/`503` for a single request. Each agent field
costs 8–10 hops and risks the 120-minute client timeout.

Two report assumptions were **disproved by logs** and shape this change: (1)
OpenRouter `400`s are **context-overflow, not json_schema** failures; (2) input
is **not** uniformly ~60k — it ranges to ~128k, so token estimation must be
real, not a fixed assumption.

## What Changes

- Add **pre-flight payload-aware candidate filtering**: estimate request input
  tokens (provider-aware tokenizer) + reserve `max_tokens`/output, then drop
  candidates whose effective context window or per-model TPM cap cannot fit the
  request *before* dispatching. Eliminates groq `413`, OpenRouter context `400`,
  and cerebras over-TPM `429` dead hops.
- Make `context_window` and per-model token caps the **source of truth** for
  filtering (currently `min_context_tokens` is never populated, so the existing
  `supports()` context check is dead code).
- **Quota-aware Gemini sibling handling** (refines shipped behavior): keep
  free-slot sibling failover only for transient RPM `429`; on
  `RESOURCE_EXHAUSTED` daily-quota `429` or `503` overload, **skip remaining free
  siblings** and go straight to paid `gemini-default` / next provider.
- **Paid fallback after free** is guaranteed: once free Gemini slots are
  exhausted/skipped, attempt `gemini-default` once before abandoning Gemini.
- **Structured-output-aware ordering**: for `json_schema` requests, prefer
  json_schema-capable providers (e.g. openrouter/mistral) earlier and demote
  providers that frequently reject the fat schema, reducing wasted strict-schema
  failovers.
- **Observability**: per-`credential` failover/cooldown attribution, a
  `quota_metric` label (rpm/tpm/rpd/context), and a per-request trace summary
  (hops, wall-clock seconds, terminal provider) so ops sees "4 keys
  enough / not enough" without grepping.

Workspace version bump: **`0.3.0-beta.16`** (from `0.3.0-beta.14`;
`0.3.0-beta.15` reserved by `github-models-provider`).

## Capabilities

### New Capabilities

- `payload-aware-routing`: Estimate request input/output tokens and pre-flight
  filter candidates by effective context window and per-model token-per-minute
  caps before dispatch.
- `structured-output-routing`: json_schema-aware candidate ordering that prefers
  reliably-capable providers and demotes frequent strict-schema rejectors.
- `routing-observability`: Per-credential and quota-metric attribution plus a
  per-request routing trace summary (hops, duration, terminal outcome).

### Modified Capabilities

- `gemini-free-multi-account`: Sibling free-slot failover becomes quota-aware —
  exhaustively tried only for transient RPM `429`; `RESOURCE_EXHAUSTED`
  daily-quota and `503` overload skip remaining free siblings and fall back to
  paid/other immediately.

## Impact

- Routing core: `ai-gateway/src/router/capability/mod.rs` (populate
  `min_context_tokens`, effective-window filter), `capability/providers.rs`
  (accurate context windows), `budget_aware/selection.rs`,
  `budget_aware/dispatch.rs`, `budget_aware/failover_loop.rs`.
- Retry classification: `ai-gateway/src/router/retry_after/classify.rs` (expose
  quota-kind to the failover loop for sibling-skip decisions).
- Token estimation: new module (provider-aware tokenizer) + dependency.
- Limits source of truth: `ai-gateway/config/embedded/provider-limits.yaml`,
  `config/provider_limits.rs` (per-model TPM/context exposed to the router).
- Metrics: `ai-gateway/src/metrics/router/` (credential + quota_metric attrs,
  trace summary).
- Docs: `docs/providers.md`, `docs/credentials.md`.
- Coordination: avoids file overlap with `github-models-provider`,
  `deepseek-web-provider`, `curated-omniroute-*` (provider catalog) and
  `health-monitor-concurrency` (`health.rs` locking) which are in flight.
- Workspace version: **`0.3.0-beta.16`**.
