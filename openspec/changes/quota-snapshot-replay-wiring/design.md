## Context

**0.5.5** shipped hierarchical `QuotaAdmission`, `QuotaSnapshot::capture`, and
`provider-stats` quota tree with live `blocked_reason`. The planner path captures
`AdmissionVerdict.blocked_reason` into `QuotaSnapshotEntry` but **never reads it** —
`QuotaSnapshot::blocked_reason()` triggers rustc `dead_code` on the struct field,
and incident replay cannot explain why candidates were excluded at plan time.

Parallel data paths today:

| Path | `blocked_reason` | `next_available_at` |
|------|------------------|---------------------|
| Live `quota_observability` (provider-stats) | ✓ re-evaluated | ✓ |
| `QuotaSnapshot` at plan time | captured, unread | **not stored** |
| `ReplayRecord` / route trace | ✗ | ✗ |
| Hop-time admit skip (failover_loop) | ✓ trace log | partial |

Operators debugging «why did autodefault skip gemini-free-3 preview?» must grep
unstructured logs or re-hit provider-stats — replay JSON is incomplete relative to
the living spec intent.

## Goals / Non-Goals

**Goals:**

- **Single source at plan time** — replay and trace consume `QuotaSnapshot`, not
  live re-evaluation.
- **Stable JSON contract** — additive fields on `ReplayRecord`; snake_case
  `blocked_reason` aligned with provider-stats and `BlockedReason` enum.
- **Plan exclusion visibility** — record quota-excluded `(credential, model)` pairs
  with reason and `next_available_at` when `quota_capacity == 0`.
- **Zero dead_code suppressions** — wire accessors; delete unused `score()` wrapper
  and redundant `gate_scope_key()`.
- **CI regression gate** — `cargo build -p ai-gateway` warns-as-error on dead_code
  for touched router plan modules.

**Non-Goals:**

- Changing admission semantics, replan logic, or scoring weights.
- Live provider-stats refactor (already correct).
- Workspace-wide clippy cleanup (`mock-server`, `upstream-emulator`) — tracked as
  optional parallel tasks, not blocking this change.
- Replay tooling UI — log contract only.

## Decisions

### D1 — Extend `QuotaSnapshotEntry` with `next_available_at`

**Decision:** Store `next_available_at: Option<DateTime<Utc>>` from `AdmissionVerdict`
at capture time; expose `QuotaSnapshot::next_available_at(credential, model)`.

**Rationale:** Replay and provider-stats already use RFC3339 instants; deriving from
`next_wait` at read time loses plan-time clock anchor and diverges from live tree.

**Alternative rejected:** Compute `captured_at + next_wait` at replay — drifts if
snapshot ages before trace emit.

### D2 — Replay score breakdown carries quota block metadata

**Decision:** Add to `ReplayScoreBreakdown`:

```rust
#[serde(skip_serializing_if = "Option::is_none")]
pub blocked_reason: Option<BlockedReason>,  // None when feasible / None variant
#[serde(skip_serializing_if = "Option::is_none")]
pub next_available_at: Option<String>,      // RFC3339
```

Populate from snapshot in `score_breakdown` / `capture_replay` when
`quota_capacity == 0.0` OR when `blocked_reason != None` (circuit/cooldown via
cooldown_secs path uses separate signal; quota block is primary).

**Rationale:** Minimal additive change; backward compatible; matches provider-stats
field names.

### D3 — New `quota_excluded` list on `PlanReplaySnapshot`

**Decision:** Add `quota_excluded: Vec<ReplayQuotaExcluded>` (cap **8** entries,
deduped by `(credential, model)`), built in `capture_replay` from pool members
that fail `feasible_for_plan` **specifically due to** `headroom_score <= 0` (not
circuit-open or zero-success — those are health exclusions, separate concern).

Each entry: `{ credential, model, blocked_reason, next_available_at, quota_capacity: 0.0 }`.

Propagate through `ReplayRecord.quota_excluded` (skip if empty).

**Rationale:** `top_alternatives` only lists feasible survivors; excluded infeasible
candidates need explicit representation for post-mortem. Cap prevents trace bloat on
large pools.

**Alternative rejected:** Stuff excluded rows into `top_alternatives` — violates
existing semantic (feasible next-best).

### D4 — Serde for `BlockedReason` in replay JSON

**Decision:** Re-use existing `BlockedReason` with `Serialize` + `snake_case`; export
through `types/extensions` via re-export or newtype wrapper if module privacy blocks.

**Rationale:** Same vocabulary as provider-stats and structured hop logs.

### D5 — Dead code cleanup without `allow`

**Decision:**

- Delete `plan/score.rs::score()` — callers use `score_breakdown` only.
- Delete `pacing/scope.rs::gate_scope_key()` — tests already assert via
  `pacing_scope_key(&resolve_pacing_scope(...))`.
- Ensure `QuotaSnapshot::blocked_reason()` and `next_available_at()` are called from
  `replay.rs` and exclusion capture.

### D6 — CI dead_code gate (scoped)

**Decision:** Add to `ai-gateway` crate root or `router/budget_aware/plan/mod.rs`:

```rust
#![deny(dead_code)]  // only if workspace allows; else CI step
```

Prefer **CI step** in existing Rust workflow:

```yaml
- run: RUSTFLAGS="-D dead_code" cargo build -p ai-gateway --lib
```

**Rationale:** Crate-wide deny may fail on unrelated legacy code; scoped build catches
regressions in main binary crate without blocking entire workspace.

**Alternative rejected:** `#allow(dead_code)` on snapshot field — violates project
quality bar.

### D7 — Version and changelog

**Decision:** Ship as **0.5.6** patch after apply; CHANGELOG section «Replay quota
block metadata».

## Risks / Trade-offs

| Risk | Mitigation |
|------|------------|
| Trace JSON size grows on large pools | Cap `quota_excluded` at 8; skip_serializing_if empty |
| Winner shows `blocked_reason` when capacity > 0 due to cooldown | Only set quota fields when `quota_capacity == 0.0` |
| `BlockedReason::Circuit` conflated with quota | Circuit exclusions omit from `quota_excluded`; health already in provider-stats |
| CI `-D dead_code` fails on unrelated code | Limit to `-p ai-gateway` build; fix only new violations in this change |

## Migration Plan

1. Deploy gateway **0.5.6** — additive JSON fields; no config changes.
2. Log consumers: read optional `blocked_reason`, `next_available_at`,
   `quota_excluded`; ignore if absent (pre-0.5.6 traces).
3. Rollback: downgrade binary; old traces remain valid.

## Open Questions

_(none — ready for apply)_
