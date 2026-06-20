## 1. Quota admission module

- [x] 1.1 Add `router/quota_admission/` with `AdmissionVerdict` and `QuotaAdmission::evaluate`
- [x] 1.2 Resolve `PacingScope` + `ResolvedModelLimits` per candidate (L0–L2 hierarchy)
- [x] 1.3 Merge pacing peek, daily headroom, cooldowns, reconcile blocks into verdict
- [x] 1.4 Unit tests `tests/quota_admission.rs` — per-model, per-slot, per-session matrix

## 2. Strict admission in plan and walk

- [x] 2.1 Refactor `QuotaSnapshot` to use `QuotaAdmission` verdicts
- [x] 2.2 Apply zero-wait rule: `next_wait > 0` ⇒ `headroom_score = 0`
- [x] 2.3 Remove sleep-probe on planned hops in `cooldown.rs`; `max_terminal_wait` for last hop only
- [x] 2.4 Hop-time re-admit in `dispatch.rs` before each upstream attempt
- [x] 2.5 Fresh snapshot on plan rebuild via `failover_loop` replan (`plan_route_chain`)
- [x] 2.6 Update `tests/budget_aware_snapshot.rs` expectations

## 3. Upstream reconcile

- [x] 3.1 Add `PacingGate::apply_upstream_reconcile(until)` on `PacingScope`
- [x] 3.2 Wire reconcile from `retry_after` + exhaustion classifiers (headers before catalog fallback)
- [x] 3.3 Align model/slot cooldown instants with reconcile `until`
- [x] 3.4 Repeat-429 guard: metric + trace when 429 hits infeasible scope
- [x] 3.5 Unit tests: header-driven reconcile, silent fallback to `cooldown-defaults`

## 4. Quota observability tree

- [x] 4.1 Extend provider-stats JSON: `accounts[]`, optional `models[]` per quota-profile
- [x] 4.2 Populate `next_available_at`, `blocked_reason` from admission state
- [x] 4.3 Root `repeat_429_violations` counter
- [x] 4.4 Update `tests/provider_observability.rs`
- [x] 4.5 Document tree shape in `docs/routing.md`

## 5. Routing load scenarios

- [x] 5.1 `admission_zero_repeat_429.rs`
- [x] 5.2 `admission_parallel_account_spread.rs` (no pool cap; parameterized N accounts)
- [x] 5.3 `admission_hop_readmit.rs`
- [x] 5.4 `admission_longcat_tpd.rs`
- [x] 5.5 `admission_per_session_deepseek.rs`
- [x] 5.6 Register scenarios in `tests/routing_load.rs` catalog

## 6. Planning hygiene and release

- [x] 6.1 Mark `proactive-quota-scheduling` superseded in `openspec/changes/README.md` or proposal cross-link
- [x] 6.2 Bump version **0.5.5** in `Cargo.toml` and `CHANGELOG.md`
- [x] 6.3 Run `mise run predeploy:rust`
- [x] 6.4 `openspec validate hierarchical-quota-admission --strict`
- [x] 6.5 Stage smoke: multi-account spread + zero repeat 429

## 7. Phase 2 spike (deferred — document only)

- [x] 7.1 Draft follow-up change `distributed-quota-state`: Redis key per `pacing_scope_key`, local cache pattern

## 8. Verify remediation

- [x] 8.1 README: mark `proactive-quota-scheduling` superseded; hierarchical ready for archive
- [x] 8.2 OpenTelemetry `gateway_repeat_429_violations_total` counter wired to repeat-429 guard
- [x] 8.3 `tests/quota_admission.rs`: per-slot (longcat), per-session (chatgpt + deepseek) matrix
- [x] 8.4 Design/spec: `quota[]` tree shape, spread ordinal (D9), Prometheus metric in observability spec
