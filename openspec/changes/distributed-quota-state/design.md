## Context

`hierarchical-quota-admission` (0.5.5) ships `QuotaAdmission` on `PacingScope` in-process.
Design D10 recorded Phase 2: Redis per scope key. This change implements that layer without
altering admission API shape (`feasible`, `blocked_reason`, reconcile semantics).

## Decisions

### D1 — Key = `pacing_scope_key(PacingScope)`

Same string as local `PacingRegistry` gate cache. `CredentialModel`, `Credential`, and `Session`
paths must not collide.

### D2 — Local cache + Redis authority

When `upstream_pacing.store: redis`:

1. `peek` / `acquire` read authoritative counters from Redis (atomic increment or Lua script).
2. Local `PacingGate` mirrors last-known state for fast reject; TTL refresh on miss.
3. When Redis unavailable, fall back to local-only mode with documented overshoot risk.

### D3 — Reconcile fan-out

`apply_upstream_reconcile(until)` writes `until` to Redis for the scope key so all replicas
mark the scope infeasible before the next plan.

### D4 — Observability unchanged

Provider-stats quota tree remains process-local. Optional follow-up: aggregate Redis headroom
for ops dashboards — out of scope here.

## Risks

| Risk | Mitigation |
|------|------------|
| Redis latency on hot path | Local cache + batch peek; configurable timeout |
| Split-brain during Redis outage | Degrade to local gates; metric `distributed_quota_degraded` |
| Key explosion (16×N models) | Same cardinality as local gates; TTL on idle keys |
