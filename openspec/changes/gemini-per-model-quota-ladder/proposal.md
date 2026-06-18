## Why

With **16× Gemini free slots** shipped, the next bottleneck is **per-model quotas
inside one AI Studio project**. Live dashboards show RPM/TPM/RPD limits per model
(e.g. Gemini 3 Flash at 38/20 RPD while Gemini 3.5 Flash and 3.1 Flash Lite on the
**same API key** still have headroom). Today the gateway treats a model-level 429
as **whole-credential failure**: pacing is one gate per `gemini-free-N`, failover
marks the entire slot dead, and `QuotaExhausted` skips all free Gemini siblings.
Operators lose a key that still has usable models. Clients also expect **stability**:
when fast preview models are exhausted, the router should **escalate to a larger /
more capable model on the same slot** to complete the request—not downgrade to a
smaller tier or abandon the slot prematurely.

## What Changes

- **Free-tier limit catalog refresh** — align embedded `provider-limits.yaml`
  Gemini free models with live AI Studio figures (e.g. preview flash RPD **20**,
  3.1 Flash Lite RPD **500**); add missing routable slugs in `providers.yaml`
  (`gemini-3.5-flash-preview`, `gemini-3.1-flash-lite`, slug↔catalog normalization).
- **Per-model pacing gates** — `PacingRegistry` resolves limits for
  `(provider, credential, upstream_model)` using catalog `model()` lookup; proactive
  reject on RPM/TPM/RPD **per model** before upstream hop.
- **Per-model cooldown state** — model-level RPM/RPD exhaustion cools
  `(credential_id, model)` not the whole credential; project-level billing cap
  still retires the entire slot.
- **Intra-slot model ladder** — ordered failover on the **same** Gemini credential:
  preferred fast models → capacity fallbacks → **stability escalation** to larger
  models (e.g. 2.5-pro) before inter-slot round-robin to `gemini-free-N+1`.
- **Sibling skip fix** — `skip_free_siblings_on_exhaustion` only when failure class
  is project-wide quota/billing, not per-model daily cap on one slug.
- **Tests** — unit tests (pacing scope, cooldown key, ladder ordering, sibling
  skip), routing_load scenarios with emulator per-model limits, regression for
  16-slot inter-credential round-robin unchanged.

## Capabilities

### New Capabilities

- `catalog-quota-pacing`: Per-model multi-dimension pacing (RPM/TPM/RPD) from
  embedded provider limits; proactive gate reject aligned to dimension boundaries.
- `gemini-per-model-quota-ladder`: Intra-slot model ladder, per-model exhaustion
  tracking, stability escalation to larger models on the same credential.

### Modified Capabilities

- `gemini-free-multi-account`: Sibling skip and cooldown semantics when only one
  model on a slot is exhausted; inter-slot round-robin after full slot ladder.
- `autodefault-intent-routing`: Stability escalation within Gemini slot complements
  global intent escalation; no downgrade below client intent floor across providers.

## Impact

- `ai-gateway/config/embedded/provider-limits.yaml`, `providers.yaml`
- `ai-gateway/src/router/pacing/` (`registry`, `limits`, `gate`, `scope`)
- `ai-gateway/src/router/budget_aware/failover_loop.rs`, `failure.rs`
- `ai-gateway/src/router/provider_attempt.rs` (cooldown key)
- `ai-gateway/src/config/provider_limits.rs` (model slug normalization)
- `crates/upstream-emulator/` — per-model limit profiles for Gemini
- `ai-gateway/src/routing_load/scenarios/`, `ai-gateway/tests/`
- `docs/credentials.md`, `docs/providers.md`
- Complements `gateway-load-acceptance` catalog-quota-pacing draft (this change
  lands the Gemini slice + ladder; does not block other providers).
