## Context

**Shipped:** `autodefault-intent-routing` (intent pool, asymmetric escalation up),
`autodefault-credential-pools` (Gemini×16, DeepSeek×2, inter-slot round-robin).

**Observed on stage (gemini-free-8):** Gemini 3 Flash RPD 38/20 while Gemini 3.5
Flash, 3.1 Flash Lite, and 2.5 Flash on the same project show zero usage and
separate RPM/TPM/RPD caps.

**Code today:**

| Layer | Location | Behavior |
|-------|----------|----------|
| Limit catalog | `config/embedded/provider-limits.yaml` | Per-model RPM/TPM/RPD under `gemini.tiers.free.models` |
| Payload cap | `provider_limits.rs::per_request_token_cap` | Per-model TPM lookup ✓ |
| Pacing | `pacing/registry.rs`, `limits.rs` | One gate per `(provider, credential_id)`; `pacing_limits_for(provider)` returns **first tier aggregate**, ignores model |
| Pacing acquire | `pacing/mod.rs::acquire_upstream_pacing` | No `model` parameter |
| Failover | `budget_aware/failover_loop.rs` | `failed_credentials.insert(credential_id)` on any failoverable failure |
| Sibling skip | `skip_free_siblings_on_exhaustion` | On `FailoverClass::QuotaExhausted`, marks **all** free Gemini siblings failed |
| Cooldown | `provider_attempt.rs` + `failure.rs` | Keyed by `credential_id` only |
| Routable models | `providers.yaml` `gemini.models` | Missing `gemini-3.5-flash-preview`, `gemini-3.1-flash-lite`; limits yaml uses `gemini-3-flash` vs API `gemini-3-flash-preview` |

`gateway-load-acceptance` already drafts `catalog-quota-pacing` but it is not in
living specs and is not implemented. This change implements the Gemini-critical
path end-to-end.

## Goals / Non-Goals

**Goals:**

1. Resolve pacing/cooldown at **(provider, tier, credential, upstream_model)** for
   Gemini free tier using embedded catalog.
2. On model-level RPM/RPD exhaustion, **ladder** to next model on the **same**
   credential before inter-slot failover.
3. **Stability escalation:** after fast-preview ladder steps, allow larger models
   on the same slot (e.g. `gemini-2.5-pro`) to satisfy client stability—never
   downgrade below `autodefault-intent-routing` floor across providers.
4. Refresh free-tier limit numbers from operator AI Studio snapshots (RPD 20 for
   preview flashes, 500 for 3.1 Flash Lite, etc.).
5. Ship unit + routing_load + emulator tests without live API keys.

**Non-Goals:**

- Paid `gemini-default` ladder (tier-3 limits differ; follow-up).
- Enabling ladders for GitHub Models / OpenRouter in **this** change (only generic
  types + YAML schema; Gemini is first data consumer).
- ChatGPT Web / DeepSeek Web (session providers keep per-session gates).
- Replacing `gateway-load-acceptance` entirely—only the Gemini pacing+ladder slice.

## Decisions

### D1 — Two-level failover hierarchy

```
Request walk (Gemini free band):

  Level A — intra-slot model ladder (same credential_id)
    3-flash → 3.5-flash → 3.1-flash-lite → 2.5-flash → [stability] 2.5-pro

  Level B — inter-slot credential round-robin (existing)
    gemini-free-8 exhausted → gemini-free-9 … gemini-free-16

  Level C — cross-provider (existing intent + cost-class)
    openrouter, groq, deepseek-web, …
```

**Rationale:** Matches Google quota model (per-model per project) and preserves
16-slot parallelism from credential-pools.

### D2 — Pacing gate key extends with model slug

Extend `gate_scope_key` (or parallel helper) to:

`(provider, credential_scope, normalized_model_slug)`

`PacingRegistry::gate_for` gains optional `model: &str`. Dispatcher passes
`candidate.capability.model` (bare slug) into `acquire_upstream_pacing`.

Limits resolved via:

```text
ProviderLimitCatalog::resolve_model_limits(provider, tier, upstream_slug)
  → candidate_slugs(slug) → tier.models lookup (shared with emulator)
```

**Alternative rejected:** Separate gate per provider only—conflates 3-flash RPD
with 3.1-lite headroom.

**Implementation note:** Slug normalization MUST reuse the same `candidate_slugs`
algorithm as `crates/upstream-emulator/src/limits/resolve.rs` (extract to
`ai-gateway` shared module or public helper) so gateway pacing and emulator stay
in sync—no provider-specific `normalize_gemini_*` helpers in router code.

### D3 — Failure classification drives retirement scope

| Upstream signal | Class | Retire |
|-----------------|-------|--------|
| RPM / transient 429 | `Transient` | **model** only; retry-after from header |
| Per-model RPD / daily | `QuotaExhausted` (model) | **model** until UTC reset |
| Project billing / "Set up billing" | `QuotaExhausted` (project) | **entire credential** + sibling skip |
| 503 overload | `Overload` | model or provider per existing rules |

Implement `FailedModelKey(credential_id, model_slug)` alongside
`failed_credentials`. `failover_loop` skips candidates matching failed model on
same credential before marking credential dead.

**Change to `skip_free_siblings_on_exhaustion`:** gate on project-level quota
detector (`looks_like_project_billing_cap(body)`), not all `QuotaExhausted`.

### D4 — Embedded model ladder config

Generic loader: `ModelLadderRegistry` from embedded
`ai-gateway/config/embedded/provider-ladders.yaml`. Each provider+tier entry
lists ordered **bands**; Gemini free is the first consumer.

```yaml
# provider-ladders.yaml (concept)
gemini:
  free:
    fast:
      - gemini-3-flash-preview
      - gemini-3.5-flash-preview
    capacity:
      - gemini-3.1-flash-lite
      - gemini-2.5-flash
    stability:
      - gemini-2.5-pro
# github-models:   # future — same schema, different slugs
#   free: { ... }
```

Router ranks candidates on the same credential by `(band_index, position)` before
cost-class tiebreak. **Stability band** is attempted only after fast+capacity
bands fail or are proactively gated out—not as first hop (client ordered speed
first, stability second).

Aligns with `autodefault-intent-routing` asymmetric escalation **up** without
violating floor: stability models must still satisfy `json_schema` when required.

### D5 — Catalog numbers from live free tier

Update `provider-limits.yaml` `gemini.tiers.free.models` to match AI Studio
(March 2026 operator snapshot):

| Catalog key | RPM | TPM | RPD |
|-------------|-----|-----|-----|
| gemini-3-flash | 5 | 250K | 20 |
| gemini-3.5-flash | 5 | 250K | 20 |
| gemini-3.1-flash-lite | 15 | 250K | 500 |
| gemini-2.5-flash | 5 | 250K | 20 |

Set `observed-at` and note in `notes` that preview models use low RPD.

**Alternative rejected:** Keep 1500 RPD—causes proactive pacing to allow traffic
that upstream will reject, wasting failover budget.

### D6 — Test strategy

Three layers; Gemini scenarios are the **first consumer**, assertions use generic
scope types where possible so GitHub Models can add cases later without new harness.

| Layer | Module / scenario | Assert |
|-------|-------------------|--------|
| **Unit** | `catalog_limit_resolve` | `candidate_slugs` maps API slug → catalog key |
| **Unit** | `pacing/registry` | `PacingScope::CredentialModel` → distinct `Arc` gates |
| **Unit** | `pacing/gate` | RPD on model A does not block model B same credential |
| **Unit** | `retry_after` / `quota_scope` | `ExhaustionScope::Model` vs `::Slot` classification |
| **Unit** | `failover_loop` | model 429 ≠ `failed_credentials`; project cap → sibling skip |
| **Unit** | `ModelLadderRegistry` | band order fast → capacity → stability |
| **routing_load** | `gemini_model_ladder_same_slot` | 3-flash exhausted → 3.1-lite same slot |
| **routing_load** | `gemini_stability_escalation` | 2.5-pro same slot before inter-slot hop |
| **routing_load** | regression | `gemini_sixteen_slot`, `failover_rpm` unchanged |
| **Emulator** | HTTP + catalog | per-model buckets; model-RPD vs project-cap 429 bodies |

**Non-goals for tests:** live AI Studio keys; combinatorial 16×N-model matrix.

## Reusable abstractions

This change ships **Gemini free** first but introduces provider-agnostic types so
GitHub Models, OpenRouter per-model tiers, and future slots reuse the same path
without `if provider == gemini` in failover.

### Quota scope hierarchy (L0 → L2)

```
L0 tier     provider + credential.tier     → limits table in provider-limits.yaml
L1 slot     credential_id | session_path  → PacingScope::Credential | ::Session
L2 model    (credential, upstream_slug)   → PacingScope::CredentialModel
```

| `ProviderQuotaProfile` (catalog metadata) | L1 scope | L2 model dimension |
|-------------------------------------------|----------|-------------------|
| `per-model` | credential_id | yes — separate gates + exhaustion |
| `per-slot` | credential_id | no — one gate per credential (legacy API providers) |
| `per-session` | session file path | no — chatgpt-web, deepseek-web |

Gemini free: `per-model`. DeepSeek Web: `per-session` (unchanged). GitHub Models:
`per-model` (future — same code path, different ladder YAML).

### Core types (new or extended modules)

| Type | Location (proposed) | Role |
|------|---------------------|------|
| `PacingScope` | `router/pacing/scope.rs` | `Session(path)` \| `Credential(id)` \| `CredentialModel(id, model)` |
| `ExhaustionScope` | `router/retry_after/quota_scope.rs` | `Model { credential, model }` \| `Slot { credential }` \| `Project { provider }` |
| `FailedModelKey` | `router/budget_aware/failover_loop.rs` or `types.rs` | Per-request walk: skip `(cred, model)` pairs |
| `resolve_model_limits()` | `config/provider_limits.rs` | Shared slug candidates + tier.models lookup (mirror emulator) |
| `ModelLadderRegistry` | `config/model_ladder.rs` | Load `provider-ladders.yaml`; `band_index(provider, tier, model)` |
| `ProviderQuotaProfile` | `config/provider_limits.rs` or `providers.yaml` | Declares per-model vs per-slot vs per-session |

**Rule:** Router and dispatcher depend on **types + catalog**, not on Gemini
string literals. Gemini-specific data lives only in embedded YAML.

### Failover walk (generic)

```
for candidate in ordered_candidates:
  if failed_model.contains(cred, model): continue
  if failed_credentials.contains(cred): continue
  dispatch → on failure:
    scope = classify_exhaustion(response)
    match scope:
      Model   → failed_model.insert(cred, model)
      Slot    → failed_credentials.insert(cred); maybe skip_siblings if Project
      Project → failed_credentials + skip_free_siblings
```

`skip_free_siblings_on_exhaustion` triggers only on `ExhaustionScope::Project`,
not on per-model `QuotaExhausted`.

### Slug resolution — single source of truth

Today duplicated:

- Gateway: `per_request_token_cap` uses bare model string
- Emulator: `candidate_slugs()` + tier lookup in `limits/resolve.rs`

**Decision:** Extract `catalog_limit_resolve(provider, tier, request_model)` into
`ai-gateway` (public or `pub(crate)`), call from:

1. `provider_limits::per_request_token_cap` / `resolve_model_limits`
2. `pacing/limits.rs`
3. `upstream-emulator` via dependency on shared helper (thin wrapper)

CI test: gateway and emulator resolve the same RPM/TPM/RPD for a fixture slug set.

### Ladder registry — provider-pluggable

`ModelLadderRegistry::band(provider, tier, model) -> Option<LadderBand>` returns
`Fast | Capacity | Stability | None`. Ranking sorts by `(band_ord, position)`.

Providers without a ladder entry: existing cost-class / budget-rank order only
(no behavior change for groq, openrouter, etc.).

### Trace / observability (generic field names)

Prefer provider-neutral trace keys with provider prefix optional:

- `quota_scope` — `model` | `slot` | `project`
- `model_ladder_band` — `fast` | `capacity` | `stability` (empty when no ladder)
- `model_ladder_position` — index within band

Gemini-specific aliases (`gemini_ladder_*`) acceptable in v1 if trace stability
requires; migrate to generic names in follow-up.

### Extension checklist (next providers)

To add GitHub Models per-model ladder later:

1. Add `github-models: quota-profile: per-model` in catalog
2. Add `provider-ladders.yaml` entry for `github-models.free`
3. Refresh `provider-limits.yaml` per-model RPM/RPD
4. Add routing_load scenario — **no** changes to `failover_loop` scope logic

## Risks / Trade-offs

- **[Risk] Slug drift** (preview suffixes) → Mitigation: normalization table + CI test that every `providers.yaml` gemini model maps to a catalog entry.
- **[Risk] Stability pro exhausts rare RPD** → Mitigation: stability band last; optional `budget_probe` skip for pro on free tier.
- **[Risk] More candidates per request** → Mitigation: proactive pacing avoids useless hops; cap ladder attempts per request (e.g. 5).
- **[Risk] Duplicate work with gateway-load-acceptance** → Mitigation: this change writes living `catalog-quota-pacing` spec; GLA can reference or defer Gemini portions.

## Migration Plan

1. Ship catalog + normalization (no behavior change until gates wired).
2. Enable per-model pacing for `gemini` + `tier: free` only (feature flag or provider guard).
3. Enable model ladder + failure scope fix.
4. Operators: no secrets change; optional doc note on ladder order.

Rollback: revert failover scope logic; pacing falls back to per-credential gate
(degraded but safe).

## Open Questions

- Whether `gemini-3.1-pro-preview` belongs in stability band for json_schema stage
  workloads (higher quality, very low RPD)—default **off** until operator confirms.
- Auto-refresh limits from AI Studio API (future); v1 stays embedded YAML with
  `observed-at`.
