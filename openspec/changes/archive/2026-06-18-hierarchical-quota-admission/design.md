## Context

The gateway already has the right **data model** but incomplete **admission enforcement**:

| Layer | Exists today | Gap |
|-------|--------------|-----|
| L0 tier limits | `provider-limits.yaml` per provider/tier | Not always driving admit decision |
| L1 account | `credentials.yaml` slots 1…N, `PacingScope::Credential` / `::Session` | Spread works; admit allows probe when wait > 0 |
| L2 model | `PacingScope::CredentialModel` for `per-model` | Snapshot stale across hops |
| Routing | `FEASIBLE → score → plan` (D16 archived design) | Feasible uses soft 3s threshold |
| Reconcile | Model cooldown + retry_after | Pacing counters may lag upstream |
| Observability | Flat `(provider, credential)` rows | No L2 model nodes; no inheritance display |

**Industrial pattern (2025–2026 LLM gateways):** hierarchical budget controls + **admission
control at the edge** + two-layer enforcement (local + distributed). Not a time-based scheduler.

Reference hierarchy already documented in `gemini-per-model-quota-ladder/design.md`:

```text
L0 tier     provider + credential.tier
L1 account  credential_id | session_path
L2 model    (credential, upstream_slug) when quota-profile: per-model
```

Admission key is **`PacingScope`**, not always `(credential, model)`.

## Goals / Non-Goals

**Goals:**

- **Catalog-driven admission** — every wait/limit from `catalog_limit_resolve` + upstream response;
  router code has no hardcoded block durations.
- **Hierarchical feasibility** — same tree for routing, reconcile, and observability; L2 inherits
  L1 when profile has no model dimension.
- **Strict admit** — infeasible candidates never HTTP on planned hops (no sleep-probe).
- **Reconcile loop** — upstream 429/headers update local state before next request.
- **Unbounded pools** — any number of configured accounts per provider; spread among feasible L1
  nodes.
- **Horizontal path** — Phase 1 correct single-process semantics; Phase 2 Redis per scope key.

**Non-Goals (Phase 1):**

- Redis distributed counters (Phase 2).
- LLM-based routing (forbidden by D15).
- Changing credential catalog slot naming convention.

## Decisions

### D1 — Name and abstraction: `QuotaAdmission`, not scheduler/oracle/plugin

**Decision:** Module `router/quota_admission/` exposes `AdmissionVerdict` for a resolved
`PacingScope` + `ResolvedModelLimits`.

**Rationale:** Matches industry «admission control» and existing `PacingScope` hierarchy. Avoids
misleading «scheduler» (cron) and «profile plugins» (optional hacks).

### D2 — Admission key = `PacingScope` resolved from catalog profile

**Decision:**

```text
scope = resolve_pacing_scope(provider, credential, model, quota_profile)
limits = catalog_limit_resolve(provider, tier, model)
verdict = admit(scope, limits, est_tokens, now)
```

| `quota-profile` | Scope key | Model dimension in admit |
|-----------------|-----------|--------------------------|
| `per-model` | `CredentialModel` | yes — separate verdict per slug |
| `per-slot` | `Credential` | no — models inherit account gate |
| `per-session` | `Session(path)` | no — deepseek/chatgpt session gate |

**Rationale:** One code path; catalog declares shape. Matches operator mental model.

### D3 — Catalog-driven dimensions only

**Decision:** `PacingLimits::resolve_for_model` + `PacingGate` windows compute `next_wait`.
Cooldown after 429 uses:

1. Upstream headers (`Retry-After`, `X-RateLimit-Reset`, `x-ratelimit-reset-*`)
2. Classifier-derived instant
3. `provider-limits.yaml` → `cooldown-defaults` / provider `cooldown:` **only if upstream silent**

**Example:** LongCat `LongCat-Flash-Lite` `tpd: 50000000` — wait from TPD window + token estimate,
not a router constant.

**Rejected:** Hardcoded «60s blocks whole key» in failover logic.

### D4 — Strict feasibility replaces `max_cooldown_wait` headroom threshold

**Decision:** `headroom_score = 0` when `next_wait > 0` OR daily headroom exhausted OR active
reconcile block OR cooldown. `max_cooldown_wait` repurposed as `max_terminal_wait` for sole
remaining candidate only.

### D5 — Three-phase request lifecycle

```text
RESOLVE  → catalog limits + PacingScope for candidate
ADMIT    → peek gates + cooldowns → feasible?
DISPATCH → HTTP only if feasible
RECONCILE→ on response, apply_upstream_reconcile(until) on scope
```

Hop-time: repeat ADMIT before each planned attempt.

### D6 — Operational routing unchanged in shape

**Decision:** Keep `FEASIBLE(c) → score(c) → plan_route_chain`. Admission replaces soft headroom
in feasibility check. Ranking (ladder band, cost-class, work-unit spread) unchanged.

### D7 — Quota observability tree mirrors admission hierarchy

**Decision:** Extend provider-stats with a top-level `quota[]` array (one node per provider):

```json
{
  "quota": [
    {
      "provider": "gemini",
      "accounts": [
        {
          "credential_id": "gemini-free-3",
          "quota_profile": "per-model",
          "blocked_until": null,
          "models": [
            { "slug": "gemini-3-flash-preview", "next_available_at": "...", "blocked_reason": "rpd" },
            { "slug": "gemini-3.1-flash-lite", "next_available_at": null, "blocked_reason": "none" }
          ]
        }
      ]
    }
  ]
}
```

For `per-slot` / `per-session`: omit `models[]` or show inherited limits from account node only.

**Rationale:** One observability tree; no separate «profile UI». Flat `providers[]` rows remain for
call counters and backward compatibility.

### D8 — Repeat-429 guard

**Decision:** If `feasible == false` at admit instant and upstream still returns 429, increment
`gateway_repeat_429_violations_total` (OpenTelemetry counter + provider-stats
`routing.repeat_429_violations`) and set `repeat_429_violation` on trace. Do not extend block
beyond existing `next_available_at`.

### D9 — Work-unit spread: hash with ordinal fallback

**Decision:** First-hop account spread uses stable hash over `(agent_name, work_unit_id, pool)`.
When `work_unit_id` ends with `-<N>` where `N` is a positive integer (e.g. `unit-1`, `unit-8`),
spread index is `(N - 1) % pool_size` among sorted feasible credential ids. This keeps
`routing_load` parallel scenarios deterministic without capping account pools.

**Rationale:** Hash alone can collide for small pools in tests; ordinal suffix is an explicit
operator convention for concurrent lane ids, not a replacement for hash on arbitrary work units.

### D10 — Phase 2 distributed admission (deferred)

**Decision:** Follow-up change `distributed-quota-state`:

- Redis key = `pacing_scope_key(scope)`
- Local `PacingGate` becomes cache; `acquire` does atomic check-and-increment in Redis when
  `upstream_pacing.store: redis` configured
- Pattern: edge-local burst + shared global quota (2026 gateway best practice)

Repo precedent: `middleware/rate_limit/redis_service.rs` for inbound limits.

## Risks / Trade-offs

| Risk | Mitigation |
|------|------------|
| Stricter admit reduces success when catalog pessimistic | Reconcile calibrates from upstream; first 429 still allowed |
| Observability payload size (16 accounts × N models) | Lazy model nodes only for per-model providers with attempts or blocks |
| Single-process fix insufficient for 15 replicas | Phase 2 explicit; document overshoot risk until Redis |
| OpenAI dynamic limits not in catalog | Reconcile from response headers; optional runtime header ingestion follow-up |

## Migration Plan

1. Ship **0.5.5** — admission + reconcile + quota tree (no Redis).
2. Operators: expect lower repeat 429; more idle accounts used when secrets exist.
3. Deprecate reference to `proactive-quota-scheduling` in planning index.
4. Phase 2 when running >1 gateway replica with shared free-tier keys.

## Open Questions

- **Q1:** Provider-stats tree — always include all configured accounts (idle included) with nested
  models, or models only when blocked/active? → **Proposed:** accounts always; models when
  per-model profile.
- **Q2:** Archive or delete `proactive-quota-scheduling` folder? → **Proposed:** leave with
  superseded note until hierarchical ships.
