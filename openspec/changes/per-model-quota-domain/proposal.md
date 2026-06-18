## Why

`provider-model-reality` (0.4.2-beta.3) fixed Gemini with a **per-model quota pattern**, but the
implementation is still Gemini-shaped: `quota-profile: per-model` only on `gemini`, ladder filter
only when a ladder exists, and **402 always maps to `Project` scope** (kills the whole credential).

Stage on beta.42 shows the same class of failure on **OpenRouter**: Nemotron hits per-model
`free-models-per-day` (50) while `gpt-oss-120b:free` still works on the same key — yet the gateway
pacing gate is **per-slot**, ranking pushes Nemotron first (alphabetical tie-break), and reactive
402 on unpaid slugs retires the entire `openrouter-default` slot. Client stability requires
**intra-provider escalation to larger capable free models**, never downgrade below intent floor.

We need **one domain model** for all `quota-profile: per-model` providers (Gemini, OpenRouter, future
GitHub Models free slugs) — not per-provider hacks. Ships in **`0.4.2-beta.4`** alongside
`rust-code-coverage` (parallel release track).

## What Changes

- Introduce **`quota-profile-domain`**: unified pacing scope, ladder-only intra-slot walk, stability
  band (escalate up), failure-signal taxonomy, and rank order driven by catalog — not alphabet.
- **Generalize** existing Gemini machinery (`ladder_filter`, `PacingScope::CredentialModel`,
  `failed_models`, `budget_probe`) to any provider declaring `quota-profile: per-model`.
- **OpenRouter first new consumer**: explicit per-slug limits in `provider-limits.yaml`, free ladder
  in `provider-ladders.yaml`, 402/429 body classification fixes, pre-dispatch paid-route skip.
- **Stability contract**: on same credential, after fast/capacity exhaustion, attempt **larger**
  free models (gpt-oss, flash-lite) before cross-provider hop; never select smaller/legacy slugs
  below client intent floor.
- **Test pyramid**: unit scope matrix, `routing_load` scenarios (OR 429 model vs 402 paid vs 200
  gpt-oss), emulator `free-models-per-day` + `402-never-purchased` profiles, optional stage smoke.
- **Defer** `gateway-load-acceptance` items 6–8 (catalog pacing + OpenRouter dual gate) to this
  change — see pointer in that proposal.

## Capabilities

### New Capabilities

- `quota-profile-domain`: Provider-agnostic per-model quota routing — profile declaration, pacing
  key, ladder-only walk, stability escalation, rank order, failure taxonomy.

### Modified Capabilities

- `per-model-exhaustion-scopes`: 402 unpaid route → `Model` on per-model profile; `free-models-per-day`
  → `QuotaExhausted` + reset-header cooldown; table row for 402 never-purchased.
- `provider-model-catalog`: OpenRouter per-slug limit entries (not shared suffix-only bucket);
  `catalog:verify-openrouter` fixture gate.
- `autodefault-intent-routing`: Intra-slot stability before cross-provider; OpenRouter ladder
  scenarios (gpt-oss before exhausted nemotron).
- `gemini-free-multi-account`: Reference quota-profile-domain ladder semantics (no Gemini-only
  special cases in code paths).

## Impact

- `provider-limits.yaml`, `provider-ladders.yaml`, `providers.yaml` (OpenRouter section)
- `router/pacing/{scope,registry}.rs`, `router/retry_after/{quota_scope,classify}.rs`
- `router/budget_aware/{ladder_filter,sort,rank}.rs`, `budget_probe/snapshot.rs`
- `routing_load/scenarios/`, `upstream-emulator` failure profiles
- `gateway-load-acceptance` proposal (pointer only; T6–T9 implementation moves here)
- Release: **0.4.2-beta.4** (CHANGELOG, Cargo.toml)

## Related Changes

| Change | Relationship |
|--------|----------------|
| [provider-model-reality](../archive/2026-06-18-provider-model-reality/) | Shipped Gemini slice; this generalizes the pattern |
| [gateway-load-acceptance](../gateway-load-acceptance/) | Items 6–8/T9 **deferred here** |
| [rust-code-coverage](../rust-code-coverage/) | Parallel release track (beta.4) |
