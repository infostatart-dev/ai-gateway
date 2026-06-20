## Why

Phase 1 admission ([hierarchical-quota-admission](../archive/2026-06-18-hierarchical-quota-admission/),
shipped **0.5.5**) is correct per process but each gateway replica keeps its own `PacingGate`
counters. With 10–15 replicas sharing free-tier API keys, aggregate traffic can exceed catalog
quotas until upstream 429s arrive. Operators need shared quota truth keyed by the same
`PacingScope` hierarchy admission already uses.

## What Changes

- Redis key = `pacing_scope_key(PacingScope)` for RPM/TPD/concurrent counters and reconcile
  `until` instants.
- `PacingRegistry` keeps a local cache; `acquire` / `peek_next_wait` consult Redis when
  `upstream_pacing.store: redis` is configured.
- Pattern: edge-local burst smoothing + shared global quota (atomic check-and-increment).
- Publish reconcile blocks from 429 classification so all replicas skip infeasible scopes.

**Explicit non-goals (this change):**

- Cross-region replication.
- Replacing process-local provider-stats tree (remains local; Redis backs admission only).

## Capabilities

### New Capabilities

- `distributed-quota-state`: Shared Redis-backed pacing counters and reconcile blocks per
  `PacingScope`.

### Modified Capabilities

- `quota-admission-control`: Admission verdicts SHALL consult distributed store when configured,
  preserving single-process semantics when Redis is disabled.

## Impact

- `router/pacing/` registry and gate integration
- Config: `upstream_pacing.store`, Redis connection (precedent: inbound `rate_limit/redis_service`)
- No change to HTTP routes or provider-stats JSON shape in v1 of this change

**Status:** deferred — propose/apply when stage replica count justifies operational cost.
