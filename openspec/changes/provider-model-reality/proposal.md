## Why

`0.4.2-beta.1` shipped per-model pacing and intra-slot model ladders, but production
metrics show ~50% Gemini hops returning **404** alongside **429**. Live
`ListModels` on a free key proves the root cause: embedded upstream slugs diverge
from the provider API (`gemini-3.5-flash-preview` does not exist; stable id is
`gemini-3.5-flash`). Worse, **404 is classified as slot exhaustion**, retiring the
entire `gemini-free-N` credential for the request walk even though only one slug
is invalid — contradicting the per-model quota model we just built.

The same class of bug will recur on every curated free provider (OpenRouter slug
drift, GitHub Models publisher prefixes, Groq renames) unless we add a
**verifiable catalog** and **quota-profile-aware failover scopes**. Clients also
require **stability**: escalate to a larger free model on the same slot to finish
the request — never downgrade below the intent floor or jump to paid slots while
free ladder models remain.

## What Changes

- **Verifiable provider model catalog** — separate `upstream_slug` (API wire id)
  from `catalog_key` (limits table key); CI/pre-deploy gate that embedded slugs
  exist in provider `ListModels` (fixture snapshots + optional live verify).
- **Quota-profile-aware exhaustion scopes** — for `per-model` providers:
  - **404 / unsupported model** → `ExhaustionScope::Model` (retire slug only)
  - **429 per-model quota** → `Model` (unchanged)
  - **503 high demand** → `Slot` cooldown on the credential (hot key), not
    project-wide sibling skip; other models on the slot may still be tried on
    later requests after cooldown
  - **Project billing cap** → `Project` (unchanged)
- **Free-tier ladder slug refresh** — fix Gemini ladder to live API ids; remove
  dead 1.5-family slugs; replace free stability band with quota-backed larger
  free models (`gemini-2.5-flash-lite`), not paid-only `gemini-2.5-pro`.
- **Intra-slot walk = ladder list only** — on `per-model` + ladder providers,
  failover on one credential walks **only** models listed in `provider-ladders.yaml`
  for that tier, not the full `providers.yaml` cartesian product.
- **Stability escalation (up only)** — stability band runs after fast/capacity
  exhaustion on the **same slot**; never selects a smaller model than the client
  intent floor; does not route to `gemini-default` while free ladder models on
  any slot remain eligible.
- **Tests** — unit scope classification, catalog verify script, routing_load
  scenarios for 404-not-slot-kill and ladder slug fix, emulator parity.

## Capabilities

### New Capabilities

- `provider-model-catalog`: Verifiable embedded model entries (`upstream_slug`,
  `catalog_key`, `last_verified_at`); CI ListModels gate; slug hygiene for free
  providers.
- `per-model-exhaustion-scopes`: Quota-profile-aware `ExhaustionScope` mapping
  for 404/400/429/503; slot vs model cooldown state; 503 high-demand slot
  cooldown.

### Modified Capabilities

- `gemini-free-multi-account`: Per-model 404 does not retire slot; 503 slot
  cooldown semantics for per-model profile; intra-slot failover uses ladder list
  only; paid `gemini-default` deferred until free ladder exhausted across slots.
- `autodefault-intent-routing`: Stability escalation within slot is upward-only;
  no downgrade to smaller models for cost; complements global intent ceiling.

## Impact

- `config/embedded/providers.yaml`, `provider-limits.yaml`, `provider-ladders.yaml`
- `config/catalog_limit_resolve.rs`, new `provider_model_catalog.rs` (or extend
  `provider_limits.rs`)
- `router/retry_after/quota_scope.rs`, `classify.rs`, `mod.rs`
- `router/budget_aware/failover_loop.rs`, `factory.rs` or selection filter
- `scripts/` or `mise.toml` catalog verify task; test fixtures under
  `ai-gateway/tests/fixtures/`
- `routing_load` scenarios; upstream-emulator 404/503 bodies
- `docs/providers.md`, `docs/credentials.md`, `CHANGELOG.md`
