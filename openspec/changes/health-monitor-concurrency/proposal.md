## Why

Upstream PR [#247](https://github.com/Helicone/ai-gateway/pull/247) explored moving the health-monitor registry to finer-grained concurrency (`DashMap`). Review surfaced a class of async bugs: iterating the map, awaiting, then re-acquiring entries without a stable handle. This fork still uses `Arc<RwLock<HashMap<...>>>`. We need a recorded direction before any container change lands.

## What Changes

- Record a product/engineering decision: **direction A** (keep `RwLock` baseline) or **direction B** (finer-grained concurrency when contention is proven).
- If **B**: document library-agnostic safety invariants for async health ticks.
- Define entry criteria for **B** (metrics/thresholds owned by the service team).
- No implementation in this change until a direction is chosen and accepted.

## Capabilities

### New Capabilities

- `health-monitor-registry`: Concurrency model and async safety invariants for the health-monitor map (spec written after decision **B**; decision-only if **A**).

### Modified Capabilities

- (none until direction **B** and invariants are approved)

## Impact

- `ai-gateway/src/router/budget_aware/health.rs` and related registry code (future PRs only).
- Review bar for any PR touching health-monitor locking or iteration across `await`.

**Upstream:** [Helicone/ai-gateway#247](https://github.com/Helicone/ai-gateway/pull/247)

**Migrated from:** `.todos/01-change-request.md`
