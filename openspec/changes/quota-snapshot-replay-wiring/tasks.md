# Tasks: quota-snapshot-replay-wiring

## 1. QuotaSnapshot model (D1)

- [x] 1.1 Add `next_available_at: Option<DateTime<Utc>>` to `QuotaSnapshotEntry`; populate from `AdmissionVerdict` in `entry_from_verdict`
- [x] 1.2 Add `QuotaSnapshot::next_available_at(credential, model) -> Option<DateTime<Utc>>` accessor
- [x] 1.3 Export snapshot helpers for integration tests if needed (`tests/budget_aware` re-exports)

## 2. Replay types (D2, D3, D4)

- [x] 2.1 Extend `ReplayScoreBreakdown` with optional `blocked_reason` and `next_available_at` (RFC3339 string); `skip_serializing_if` when absent
- [x] 2.2 Add `ReplayQuotaExcluded` struct `{ credential, model, blocked_reason, next_available_at, quota_capacity }`
- [x] 2.3 Extend `PlanReplaySnapshot` and `ReplayRecord` with `quota_excluded: Vec<ReplayQuotaExcluded>` (default empty)
- [x] 2.4 Re-export or wire `BlockedReason` for serde in `types/extensions` (public JSON surface)

## 3. Replay capture wiring (D2, D3)

- [x] 3.1 Pass snapshot quota metadata into `ScoreInput` / `score_breakdown` when `headroom == 0.0`
- [x] 3.2 Implement `capture_quota_excluded(ctx, pool, survivors)` — pool members with `headroom_score <= 0`, cap 8, dedupe keys
- [x] 3.3 Wire `capture_replay` to populate winner/alternatives block fields and `quota_excluded`
- [x] 3.4 Update `build_replay_record` to propagate `quota_excluded`

## 4. Dead code cleanup (D5)

- [x] 4.1 Delete unused `plan/score.rs::score()` function and `#allow(dead_code)`
- [x] 4.2 Delete `gate_scope_key()` from `pacing/scope.rs`; adjust unit test to use `pacing_scope_key` only
- [x] 4.3 Verify `cargo build -p ai-gateway` has zero dead_code warnings on plan/snapshot modules

## 5. Tests

- [x] 5.1 `tests/budget_aware_snapshot.rs`: RPM/RPD blocked candidate asserts `blocked_reason` + `next_available_at` accessors
- [x] 5.2 `tests/replay_record.rs`: winner with `quota_capacity: 0` serializes `blocked_reason`; feasible winner omits fields; `quota_excluded` shape
- [x] 5.3 `tests/quota_admission.rs`: optional cross-check snapshot capture uses same verdict as direct evaluate
- [x] 5.4 Unit test in `plan/replay.rs` or `plan/mod.rs`: pool with mixed feasible/infeasible → `quota_excluded` count and reasons

## 6. CI and release gate (D6, D7)

- [x] 6.1 Add CI step `RUSTFLAGS="-D dead_code" cargo build -p ai-gateway --lib` (or document in existing workflow)
- [x] 6.2 `cargo ci-clippy` clean on touched modules
- [x] 6.3 `cargo test -p ai-gateway --test budget_aware_snapshot --test replay_record --test quota_admission --all-features`
- [x] 6.4 CHANGELOG `[0.5.6]` — replay quota block metadata
- [x] 6.5 `openspec validate quota-snapshot-replay-wiring --strict`

## 7. Optional — workspace clippy hygiene (non-blocking)

- [ ] 7.1 Fix clippy `-D warnings` in `crates/upstream-emulator` (3 items)
- [ ] 7.2 Fix clippy in `mock-server` and `scripts/test` crates (14 items)
- [ ] 7.3 Extend CI to run workspace clippy beyond `ai-gateway` only
