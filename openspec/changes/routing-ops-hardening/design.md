## Context

`client-context-route-planning` (0.5.0.1) added `CallerRequestContext`, route memory,
and `CredentialHealthRegistry`. Today:

- `work_unit_id` is `None` without explicit/session headers → spread and memory skip.
- Circuit-open state lives only in-process; provider-stats shows attempts but not
  **why** a credential stopped receiving traffic.
- Replay/trace uses `q_headroom` — operators read it as HTTP/provider metric; it is
  **gateway quota capacity at plan time** (pacing + catalog).

Invokers (Graphiti) may not send session headers yet. Gateway already assigns
`X-Request-Id` via `MakeRequestId` on the HTTP stack.

## Goals / Non-Goals

**Goals:**

- Always resolve a work unit for router requests (spread + trace never "blind").
- Document header contract for deploy / CHANGELOG.
- Surface circuit + planner exclusion in provider-stats with tests.
- Rename observability fields to **quota capacity** (clear operator language).
- Confirm stability-up scenarios remain green (no regression).

**Non-Goals:**

- Redis route memory (Phase 2 — see Open Questions).
- Changing sticky memory semantics or stability ladder algorithm.
- Invoker driver implementation (separate repo).

## Decisions

### D1 — Work unit resolution ladder

```
1. X-Work-Unit-Id          → source: explicit
2. Helicone-Session-Id     → source: helicone-session
3. X-Request-Id            → source: request-id
4. (fallback) uuid v4      → source: generated   # only if request-id missing
```

**Rationale:** `request-id` gives **per-request spread** for anonymous parallel traffic
(better than all `unknown-invoker` colliding). It does **not** replace conversational
`session_id` for multi-turn sticky — invoker must still send session header for that.

**Rejected:** Default `work_unit_id = agent_name` — one lane per agent, worse under
parallel load.

**Rejected:** Always random UUID without echo — breaks observability correlation; ladder
uses existing `X-Request-Id` first.

`CallerRequestContext` gains `work_unit_source: WorkUnitSource` enum for trace/replay.

Optional: echo `X-Work-Unit-Id` response header when source is `generated` or
`request-id` (config-gated, default on for router routes).

### D2 — Sticky memory (no refactor)

Sticky memory is **intentional**: same `(agent, work_unit)` → prefer last successful
binding. Good for sequential calls in one session. Bad only when many **parallel** calls
share one work unit — mitigated by invoker concurrency cap (documented).

**No code change** in this change; add FAQ to routing.md.

### D3 — Provider-stats health merge

Extend `ProviderStatsRow` (or nested `routing_health` object):

```json
{
  "credential": "gemini-free-8",
  "calls": { "attempts": 12, "success": 1 },
  "routing_health": {
    "circuit_open": true,
    "open_until": "2026-06-19T12:00:00Z",
    "success_rate": 0.08,
    "planner_excluded": true
  }
}
```

`planner_excluded = circuit_open || credential_zero_success_dead`.

Snapshot built in `snapshot_with_credentials` by querying `CredentialHealthRegistry`
for each configured credential row (including idle).

**Rejected:** Separate `/routing-health` endpoint for v1 — one operator surface is enough.

### D4 — Terminology: quota capacity

| Old (internal/trace) | New (operator-facing) |
|----------------------|------------------------|
| `q_headroom` | `quota_capacity` |
| `headroom_score` in docs | quota capacity score (0–1) |

JSON: add `quota_capacity`; keep `q_headroom` as deprecated alias for one minor release
(serde alias or duplicate field).

### D5 — Stability-up verification

No algorithm change. Task: run `routing_load` scenarios
`stability_escalation_plan`, `stability_never_downgrade` + unit planner tests; document
in tasks as gate.

### D6 — Redis route memory (Phase 2, not this change)

Future `route-memory-redis` change:

- `RouteMemoryStore` trait: `InProcess` (moka, default) | `Redis` (key prefix
  `ai-gw:route-memory:`).
- Startup: if Redis configured and `route_memory.redis.enabled`, log welcome line:
  `Redis cache: active; Redis route memory: active`.
- On gateway process start with new `instance_id`: optional `FLUSHDB` only for route
  memory prefix (never global Redis) — **dangerous**, needs explicit config
  `route_memory.redis.flush_on_startup: false` default.
- Tests: integration with test Redis container.

**Deferred** — user priority is D1 deploy safety first.

## Risks / Trade-offs

| Risk | Mitigation |
|------|------------|
| Synthetic `request-id` work units prevent cross-request sticky | Document: session header required for multi-turn sticky |
| Operators confuse circuit-open with credential removed from config | `routing_health` fields + docs |
| JSON rename breaks dashboards | Deprecated alias `q_headroom` one release |
| Health merge adds snapshot cost | O(credentials) query, same as idle merge |

## Migration Plan

1. Deploy gateway 0.5.0.1+ with this change.
2. No config migration required; behaviour improves for callers without headers.
3. Update Grafana panels: use `routing_health.circuit_open`, `quota_capacity`.
4. Invoker team: still schedule `session_id` → `X-Work-Unit-Id` for full sticky.

## Open Questions

- Echo `X-Work-Unit-Id` on response by default or opt-in? **Proposal: default on for
  router routes only.**
- Phase 2 Redis: shared flush policy with existing Redis rate-limit DB index?
