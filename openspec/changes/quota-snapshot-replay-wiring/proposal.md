## Why

Release **0.5.5** shipped hierarchical `QuotaAdmission` and captures `blocked_reason` in
`QuotaSnapshot` at plan time, but **nothing reads it** — rustc reports dead_code on
`snapshot.rs`, and incident replay cannot explain *why* a candidate had zero quota capacity.
Live provider-stats already exposes `blocked_reason` via `quota_observability` (re-evaluated at
request time); the **planner snapshot path is half-wired**, violating the observability contract
in `routing-observability` and leaving operators blind during post-mortem replay.

This is a **0.5.6 quality patch**: complete the data plane from admission verdict → plan snapshot
→ replay/trace/tests — without new router behaviour, `allow(dead_code)`, or duplicate live
re-evaluation.

## What Changes

1. **Replay contract** — `ReplayScoreBreakdown` and excluded alternatives carry
   `blocked_reason` (and optional `next_available_at`) from `QuotaSnapshot` when
   `quota_capacity == 0`.
2. **Planner exclusion trace** — route trace records why candidates were dropped at plan time
   (`planner_excluded_reason`) for infeasible hops, distinct from hop-time admit skips.
3. **Dead code cleanup** — remove `#allow(dead_code)` on `score()` wrapper and
   `gate_scope_key()` while wiring snapshot accessors; no new suppressions.
4. **Tests** — `budget_aware_snapshot`, `quota_admission`, `replay_record` assert
   `blocked_reason` round-trip; CI gate: `cargo build -p ai-gateway` with zero dead_code warnings
   on touched modules.
5. **Lint hygiene follow-up** (same change, separate tasks section): workspace clippy fixes in
   `mock-server`, `scripts/test`, `upstream-emulator` — optional parallel track, not blocking
   snapshot wiring.

**Explicit non-goals:**

- Changing admission semantics (feasible/skip/re-admit) — already shipped 0.5.5.
- Duplicating `quota_observability` live tree in snapshot — snapshot is plan-time only.
- Redis / distributed quota (Phase 2 `distributed-quota-state`).

## Capabilities

### New Capabilities

_(none — wiring existing capabilities)_

### Modified Capabilities

- `routing-observability`: ReplayRecord and alternatives SHALL include plan-time
  `blocked_reason` / `next_available_at` from quota snapshot when capacity is zero.
- `quota-admission-control`: QuotaSnapshot `blocked_reason` SHALL be consumed by planner replay
  and tests; write-only capture is non-compliant.
- `route-chain-planning`: Plan-time exclusion logging SHALL reference snapshot blocked reason for
  infeasible candidates.

## Impact

- **Code:** `plan/snapshot.rs`, `plan/replay.rs`, `plan/build.rs`, `trace.rs`,
  `types/extensions.rs`, `tests/budget_aware_snapshot.rs`, `tests/replay_record.rs`,
  `tests/quota_admission.rs`; minor cleanup in `plan/score.rs`, `pacing/scope.rs`.
- **API:** additive JSON fields on route trace / ReplayRecord (`blocked_reason`, optional
  `next_available_at` on score breakdown) — backward compatible.
- **CI:** add dead_code check step or `#![deny(dead_code)]` scoped to router plan module (design
  decision).
- **Version:** patch **0.5.6** after apply.
