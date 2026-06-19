## Why

Release **0.5.0.1** shipped caller-aware route planning, but production operators still
see confusing behaviour when invokers omit session headers, when credentials silently
disappear from the plan (circuit-open), and when observability uses opaque terms like
`headroom`. Without defaults and visible health state, the planner looks broken even
when it is working as designed.

## What Changes

1. **Default work-unit resolution** — when `X-Work-Unit-Id` / `Helicone-Session-Id` are
   absent, derive a synthetic work unit from `X-Request-Id` (or generate one) so hash
   spread and trace attribution always run; record `work_unit_source` in trace.
2. **Deploy documentation** — CHANGELOG upgrade notes + `docs/routing.md` table for
   which header to send when; echo resolved work unit in route trace (optional response
   header).
3. **Routing health in provider-stats** — expose per-credential `circuit_open`,
   `success_rate`, `planner_excluded` (and `open_until` when open) from
   `CredentialHealthRegistry` merged into `GET /v1/observability/provider-stats`.
4. **Terminology** — rename user-facing `headroom` / `q_headroom` to **quota capacity**
   in provider-stats routing blocks, route trace, and `ReplayRecord` JSON (keep backward
   alias one release if needed).
5. **Tests** — unit + `routing_load` / provider-stats integration for defaults and
   circuit visibility.

**Explicit non-goals (this change):**

- **Redis-backed route memory** — follow-up change `route-memory-redis` (Phase 2); prod
  Redis exists for rate-limit/budget today, not route memory.
- **Invoker driver (Graphiti)** — remains out of repo; defaults reduce pain but true
  multi-turn sticky still needs `session_id` → `X-Work-Unit-Id`.
- **Sticky memory refactor** — behaviour is correct; document only (no code change).
- **Stability ladder logic** — already covered by `routing_load`; verify scenarios stay
  green, no algorithm change.

## Capabilities

### New Capabilities

_(none — operational hardening of existing capabilities)_

### Modified Capabilities

- `caller-request-context`: synthetic default work unit ladder; `work_unit_source`;
  trace echo of resolved id.
- `routing-observability`: credential health fields on provider-stats rows; quota
  capacity naming in trace/replay JSON.

## Impact

- `middleware/caller_context/` — resolution ladder, source metadata
- `metrics/provider/runtime.rs` — merge health into snapshot rows
- `router/budget_aware/health_registry.rs` — query API for snapshot
- `types/extensions.rs` — `work_unit_source`, replay field rename
- `docs/routing.md`, `CHANGELOG.md` — deploy / header contract
- `tests/caller_context.rs`, `tests/provider_observability.rs`, routing_load scenarios
