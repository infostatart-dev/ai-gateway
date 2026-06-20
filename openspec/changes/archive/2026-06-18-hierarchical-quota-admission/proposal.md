## Why

Release **0.5.x** shipped plan-then-walk routing, but production still shows concentrated traffic,
~20% failover, and repeat upstream **429** on pairs the gateway already knew were exhausted.
Root cause is not «нет шедулера» — it is **weak admission control**: the router probes upstream
when local quota state says wait, uses stale snapshots across ladder hops, and does not reconcile
counters after upstream truth (reset headers, `free-models-per-day`, RPM).

Operators need an **industrial admission pattern**: declarative limits from
`provider-limits.yaml` + credentials catalog, hierarchical scope (provider → account → model),
**feasible-then-rank** routing, and observability that mirrors the same tree. Pool size must scale
from 1 to N accounts per provider without code caps; horizontal gateway replicas (10–15) require a
clear path to shared quota state (Phase 2).

This change **supersedes** draft `proactive-quota-scheduling` (scheduler/oracle framing was wrong).

## What Changes

### Phase 1 — Admission control plane (ships **0.5.5**)

1. **`QuotaAdmission`** on `PacingScope` (L0–L2 hierarchy) — `feasible(scope, est_tokens)` and
   `next_available_at` from catalog-resolved dimensions (RPM, TPM, RPD/TPD, concurrent,
   min-interval) plus exhaustion cooldowns. **No magic constants** in router code; limits from
   `catalog_limit_resolve` (e.g. LongCat `tpd: 50000000` per model slug).
2. **Strict admission** — `peek_next_wait > 0` ⇒ candidate not feasible; no sleep-probe on planned
   hops; skip without HTTP attempt counter.
3. **Hop-time re-admit** — re-evaluate feasibility before each planned upstream attempt; ladder
   hops consume quota between peeks.
4. **Reconcile on upstream response** — after 429/classified exhaustion, align pacing block +
   cooldown to upstream `T` (headers first, `cooldown-defaults` in catalog only as fallback).
5. **Repeat-429 guard** — upstream 429 on a pair already infeasible at admit time = gateway
   violation (metric + trace).
6. **Quota observability tree** — `GET /v1/observability/provider-stats` exposes provider →
   account (credential/session) → model nodes when L2 applies; models inherit account limits when
   no per-model gate exists.
7. **Tests** — admission matrix per `quota-profile`, routing_load: zero-repeat-429, parallel spread
   across N accounts, hop re-admit after first 429, LongCat catalog-derived TPD scenario.

### Phase 2 — Distributed admission (deferred follow-up `distributed-quota-state`)

- Shared counters in Redis per `PacingScope` key for horizontally scaled gateways (local peek =
  cache, global authority = atomic increment). Repo already uses Redis for inbound rate limits;
  upstream pacing reuses the same pattern.

**Explicit non-goals (Phase 1):**

- Invoker / Graphiti changes.
- KPI «zero primary 429 from upstream» — goal is **≈0 repeat** on infeasible pairs.
- Hard cap on credential pool size in code or YAML catalog slots.

## Capabilities

### New Capabilities

- `quota-admission-control`: Hierarchical admission on `PacingScope` (L0 tier → L1 account → L2
  model); catalog-driven dimensions; feasible filter; hop re-admit; upstream reconcile; repeat-429
  guard.

### Modified Capabilities

- `quota-headroom-scheduling`: Headroom derived from admission feasibility; zero-wait rule; fresh
  snapshot on replan.
- `route-chain-planning`: Plan only feasible candidates; ladder filtered by admission; replan with
  fresh quota state.
- `per-model-exhaustion-scopes`: Reconcile pacing block from classified 429; no router hardcoded
  cooldown durations.
- `routing-observability`: Quota tree in provider-stats (`accounts[]`, optional `models[]`,
  `next_available_at`, `blocked_reason`, `repeat_429_violations`).
- `routing-load-verification`: Admission scenarios per quota-profile; LongCat TPD; multi-account
  spread without pool cap.

## Impact

- `router/quota_admission/` (new) — admission evaluator on `PacingScope`
- `router/pacing/{scope,gate,registry}.rs` — `apply_upstream_reconcile`, strict peek semantics
- `router/budget_aware/plan/{snapshot,build}.rs`, `dispatch.rs`, `cooldown.rs`
- `router/retry_after/` — reset instant → reconcile
- `metrics/provider/runtime.rs` — quota tree rows
- `config/catalog_limit_resolve.rs`, `provider-limits.yaml` (docs only unless gaps)
- `tests/budget_aware_snapshot.rs`, `tests/rl/scenarios/`
- `CHANGELOG.md`, `Cargo.toml` → **0.5.5**

## Related Changes

| Change | Relationship |
|--------|----------------|
| `proactive-quota-scheduling` | **Superseded** — same bugs, wrong scheduler/oracle framing |
| `per-model-quota-domain` (0.4.2-beta.4) | L2 model scope + OpenRouter; admission hardens it |
| `gemini-per-model-quota-ladder` | L0–L2 hierarchy source; admission implements it fully |
| `routing-ops-hardening` (0.5.1) | Extends observability with quota tree |
| `distributed-quota-state` (future) | Phase 2 Redis shared counters |
