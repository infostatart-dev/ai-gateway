## Context

**Shipped (0.4.2-beta.1):** `gemini-per-model-quota-ladder` — per-model pacing gates,
`ModelLadderRegistry`, `ExhaustionScope`, `failed_models` set, ladder ranking in
`sort.rs`.

**Observed on stage:** eight Gemini free slots each ~17–22 hops, ~50% **404**, ~50%
**429**, 0% Gemini success. Live API probe on `gemini-free-9`:

| Slug in embedded config | Live API | generateContent |
|-------------------------|----------|-----------------|
| `gemini-3.5-flash-preview` | absent | **404** |
| `gemini-3.5-flash` | present | 503 (exists) |
| `gemini-3-flash-preview` | present | 200 |
| `gemini-3.1-flash-lite` | present | 200 |
| `gemini-2.5-pro` | present | 429 billing (no free quota) |
| `gemini-1.5-*` (3 slugs) | absent | would 404 |

**Code gap today:**

| Layer | Location | Problem |
|-------|----------|---------|
| Catalog | `providers.yaml` | `upstream_slug` = wire id; no verify; phantom slugs |
| Limits resolve | `catalog_limit_resolve.rs` | Strips `-preview` for **limits only**; upstream still sends preview |
| Candidate factory | `budget_aware/factory.rs` | Cartesian product: every credential × every `providers.yaml` model |
| Exhaustion scope | `quota_scope.rs` | 404 → `Transient` → **Slot** (kills whole credential) |
| 503 class | `retry_after/mod.rs` | `Overload` → sibling skip (legacy per-slot semantics) |
| Free ladder | `provider-ladders.yaml` | `gemini-3.5-flash-preview`, `gemini-2.5-pro` on free tier |

`curated-free-providers-expansion` explicitly deferred live catalog sync. This
change adds **verify-at-build-time** without runtime ListModels on every request.

## Goals / Non-Goals

**Goals:**

1. **Three-layer model identity** reusable across providers:
   `catalog_key` (limits) · `upstream_slug` (wire) · `display_name` (ops).
2. **Quota-profile-aware scopes** — `per-model` providers retire `(cred, slug)`
   on 404/unsupported; `503 high demand` cools the **slot** briefly.
3. **Ladder-only intra-slot walk** — failover on one Gemini free credential tries
   only `provider-ladders.yaml` models, ordered fast → capacity → stability.
4. **Stability = escalate up on same slot** — larger free models (`2.5-flash-lite`,
   `3.1-flash-lite`) after fast band; never downgrade below client intent floor;
   no hop to `gemini-default` while free ladder models remain on any slot.
5. **CI catalog gate** — embedded `upstream_slug` values must appear in frozen
   ListModels fixtures (Gemini first consumer; OpenAI-compat pattern documented).
6. **Test pyramid** — unit scope tests, catalog verify script, routing_load 404/
   ladder scenarios, emulator body fixtures.

**Non-Goals:**

- Runtime dynamic catalog refresh on every request (admin “refresh models” UI).
- Paid `gemini-default` ladder redesign (tier-3 limits differ).
- Rewriting all eight Tier-1 free providers in one pass (framework + Gemini
  first; OpenRouter/GitHub verify tasks stubbed).
- Changing session providers (ChatGPT/DeepSeek Web) — `per-session` profile unchanged.

## Decisions

### D1 — Model identity: `upstream_slug` ≠ `catalog_key`

```yaml
# providers.yaml (concept)
gemini:
  models:
    - upstream: gemini-3.5-flash
      catalog: gemini-3.5-flash
    - upstream: gemini-3-flash-preview
      catalog: gemini-3-flash
      aliases: [gemini-3-flash]   # client model= normalization only
```

- **Wire:** dispatcher sends `upstream` to Google.
- **Limits:** `catalog_limit_resolve` uses `catalog` key in `provider-limits.yaml`.
- **Rationale:** Google graduated `3.5-flash-preview` → `gemini-3.5-flash` (GA May
  2026); limits table already uses bare keys.

**Alternative rejected:** Keep single string + `candidate_slugs` normalization
only — hides wire mismatch; caused production 404.

### D2 — Exhaustion scopes depend on `ProviderQuotaProfile`

```
                    ┌─────────────────────────────────────┐
                    │ classify(status, body, quota_profile)│
                    └──────────────────┬──────────────────┘
                                       │
          per-model                    │                    per-slot / per-session
          ─────────                    │                    ─────────────────────
          404 NOT_FOUND ──► Model      │                    404 ──► Slot (legacy)
          400 unsupported ► Model      │
          429 RPM ────────► Model      │                    429 ──► Slot or Project
          429 RPD model ──► Model      │
          429 billing ────► Project    │
          503 high demand ► Slot *     │                    503 ──► Slot + sibling skip
```

\* **503 on per-model providers:** credential enters **short slot cooldown**
(`provider-error` or new `high-demand` override, e.g. 30–60s). For the **current
request walk**, insert `failed_credentials` only if body matches high-demand
pattern AND no other ladder model on same slot remains — otherwise retire only
the attempted model and continue ladder.

`classify_exhaustion_scope` gains `quota_profile: ProviderQuotaProfile` parameter
threaded from `ProviderLimitCatalog::quota_profile(provider)`.

### D3 — 404 and unsupported model never retire slot on per-model profile

Extend `classify_exhaustion_scope`:

```rust
// per-model profile
NOT_FOUND | BAD_REQUEST(unsupported) → ExhaustionScope::Model
```

`failover_loop` already maps `Model` → `failed_models.insert` only.

Add **long model cooldown** for 404 (e.g. 24h or until restart) — phantom slug
should not retry every request.

### D4 — 503 “high demand” = hot slot, not immediate paid fallback

Detect body pattern: `high demand`, `try again later` (Gemini 503 probe).

| Profile | Request walk | Persistent state |
|---------|--------------|------------------|
| `per-model` | If same-slot ladder has more models → `Model` or continue; else `Slot` cooldown | `credential_states.cooldown_until` |
| `per-slot` (legacy) | Keep sibling skip per `gemini-free-multi-account` | unchanged |

**Rationale:** User requirement — 503 signals key-level pressure; cooldown slot,
don't burn all 16 siblings in one request.

### D5 — Intra-slot walk = ladder list only

Today `factory.rs` builds `credentials × providers.models` (11 × 16 = 176 Gemini
candidates). Failover sorted list includes dead slugs (`1.5-*`) and paid-tier
models not in free ladder.

**Decision:** For providers with `quota-profile: per-model` **and** a ladder entry,
`ordered_candidates` filters to:

```
ladder_slugs(provider, tier) ∩ capability_eligible ∩ intent_floor
```

per credential before ranking. Inter-slot order unchanged (round-robin).

**Paid `gemini-default`:** attempted only after **all** free credentials exhaust
their ladder walks for the request — not mixed into intra-slot steps.

### D6 — Free stability band refresh (client stability, no paid hop)

Replace free stability band:

```yaml
# provider-ladders.yaml (target)
gemini:
  free:
    fast:
      - gemini-3-flash-preview      # still live; Computer Use path
      - gemini-3.5-flash          # NOT -preview
    capacity:
      - gemini-3.1-flash-lite     # 500 RPD workhorse
      - gemini-2.5-flash
      - gemini-2.5-flash-lite     # larger free flash; live API ✓
    stability:
      - gemini-2.5-flash-lite     # repeat ok — heaviest free text-out with quota
```

Remove `gemini-2.5-pro` from **free** ladder (0/0/0 in AI Studio; 429 billing on
probe). Stability = **upward** within free quota — `2.5-flash-lite` after fast
preview exhaustion, not downgrade to smaller tier.

`autodefault-intent-routing` floor still blocks cross-provider downgrade.

**Alternative rejected:** Keep `2.5-pro` in stability — burns hops on 429, user
never wanted paid models inside free slot walk.

### D7 — Catalog verification (CI, not runtime)

```
mise run catalog:verify-gemini
  → load fixture tests/fixtures/gemini-listmodels.json (frozen from ListModels)
  → assert every providers.gemini upstream_slug ∈ fixture
  → assert every provider-ladders slug ∈ fixture
  → fail CI if mismatch
```

Optional `CATALOG_VERIFY_LIVE=1` + secrets for manual/pre-release refresh of
fixture. Pattern documented for OpenAI-compat providers (`GET /v1/models`).

**DeltaLLM / eneo pattern:** static embedded + mandatory verify gate.

### D8 — Test strategy (architectural)

| Layer | What | Assert |
|-------|------|--------|
| **Unit** | `quota_scope` + profile | 404+per-model→Model; 404+per-slot→Slot |
| **Unit** | `looks_like_high_demand` | Gemini 503 body → slot class |
| **Unit** | `ladder_candidate_filter` | only ladder slugs per cred |
| **Script** | `catalog:verify-gemini` | no phantom slugs in YAML |
| **routing_load** | `gemini_404_retires_model_not_slot` | 404 on 3.5 slug → 3.1-lite same slot |
| **routing_load** | `gemini_ladder_live_slugs` | emulator uses `gemini-3.5-flash` |
| **routing_load** | `gemini_stability_escalates_up` | fast exhausted → 2.5-flash-lite not 1.5 |
| **Emulator** | 404/503 bodies | parity with Google error shapes |

## Risks / Trade-offs

| Risk | Mitigation |
|------|------------|
| Fixture stale when Google adds models | `last_verified_at` in YAML; CI warning after N days |
| 503 model vs slot ambiguity | Body classifier + prefer ladder continuation |
| Breaking `providers.yaml` shape | Support legacy string-only entries during migration |
| Fewer candidates after ladder filter | Intended — reduces hop noise and 404 rate |
| `gemini-flash-latest` aliases | Document but don't route by default (unpredictable pacing) |

## Migration Plan

1. Ship slug hotfix + scope fix in `0.4.2-beta.2` (this change).
2. Migrate `providers.yaml` gemini models to structured entries (or alias map).
3. Remove `gemini-1.5-*`, `gemini-3.5-flash-preview` from embedded config.
4. Add CI verify task to `predeploy:rust` when gemini/openrouter YAML touched.
5. Monitor stage: 404 rate on Gemini should drop sharply; eff.% should rise within minutes.

## Open Questions

- **503 on per-model:** continue ladder on same slot in same request, or only
  cooldown and retry next request? **Proposed:** continue ladder if models remain;
  slot cooldown applies to pacing gate for all models on that credential.
- **Permanent 404 ban:** in-memory for process lifetime vs persisted? **Proposed:**
  long model cooldown (24h) matching phantom slug semantics.
- **Rolling aliases** (`gemini-flash-latest`): defer to follow-up unless operator
  wants them in fast band.
