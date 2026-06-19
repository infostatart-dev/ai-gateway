# Tasks: routing-ops-hardening

## 1. Default work unit resolution (priority)

- [x] 1.1 Add `WorkUnitSource` enum and extend `CallerRequestContext` with `work_unit_source`
- [x] 1.2 Implement resolution ladder in `caller_context/parse.rs`: explicit → session → `X-Request-Id` → generated UUID
- [x] 1.3 Update `CallerContextLayer` to always set non-empty `work_unit_id` on router routes
- [x] 1.4 Propagate `work_unit_source` through `PendingRouteTrace` / route trace logs
- [x] 1.5 Optional: echo resolved `X-Work-Unit-Id` on router responses (config default on)
- [x] 1.6 Unit tests `tests/caller_context.rs`: each ladder step + source enum
- [x] 1.7 `routing_load` scenario: anonymous parallel requests spread when only `X-Request-Id` differs

## 2. Deploy documentation

- [x] 2.1 Update `docs/routing.md` — header table, synthetic fallback, sticky vs spread FAQ
- [x] 2.2 Add CHANGELOG `[0.5.0.2]` or amend `0.5.0.1` upgrade notes with header contract
- [x] 2.3 Sticky memory FAQ: intentional behaviour, concurrency guidance (no code change)

## 3. Provider-stats routing health

- [x] 3.1 Add `RoutingHealthSnapshot` struct and `routing_health` on `ProviderStatsRow`
- [x] 3.2 Expose `CredentialHealthRegistry` query helpers: `circuit_open`, `success_rate`, `open_until`
- [x] 3.3 Merge health into `snapshot_with_credentials` for all configured rows (idle + active)
- [x] 3.4 Unit tests: circuit-open row shows `planner_excluded=true`
- [x] 3.5 Extend `tests/provider_observability.rs` for `routing_health` JSON shape

## 4. Quota capacity terminology

- [x] 4.1 Rename `ReplayScoreBreakdown.q_headroom` → `quota_capacity` with `#[serde(alias = "q_headroom")]`
- [x] 4.2 Update route trace / replay log field names and `docs/routing.md` prose
- [x] 4.3 Update `tests/replay_record.rs` assertions for new field name + alias

## 5. Stability-up verification (no algorithm change)

- [x] 5.1 Run `routing_load` `stability_escalation_plan` + `stability_never_downgrade` — must pass
- [x] 5.2 Document in design/tasks that stability ladder is unchanged; failures block release

## 6. Quality gate

- [x] 6.1 `cargo ci-clippy` clean on touched modules
- [x] 6.2 `cargo test -p ai-gateway --lib --features testing` + targeted integration tests
- [x] 6.3 `openspec validate routing-ops-hardening --strict`

## Deferred (Phase 2 — separate change `route-memory-redis`)

- Redis `RouteMemoryStore` backend, startup welcome log, prefix-scoped flush policy, Redis integration tests
