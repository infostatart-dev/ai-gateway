## 1. Caller request context

- [ ] 1.1 Add `CallerRequestContext` to `types/extensions.rs` (`agent_name`, `work_unit_id`)
- [ ] 1.2 Implement `middleware/caller_context/` — parse `X-Agent-Name`, `Helicone-Property-Agent`, `X-Work-Unit-Id`, `Helicone-Session-Id`
- [ ] 1.3 Wire `CallerContextLayer` into router stack in `app/stack.rs` or `router/meta.rs`
- [ ] 1.4 Unit tests: header precedence and missing defaults (`caller_context` module)

## 2. Credential health registry

- [ ] 2.1 Add `CredentialHealthRegistry` in `router/budget_aware/health_registry.rs` (rolling window, circuit state)
- [ ] 2.2 Hook health updates from `metrics/provider/runtime.rs` `record_attempt`
- [ ] 2.3 Implement circuit-open rules (window min 5, success <10%, 401 immediate, slot-all-models cooldown)
- [ ] 2.4 Expose `is_circuit_open`, `success_rate`, `provider_zero_success` queries for planner
- [ ] 2.5 Unit tests: circuit open/close, window rollover, auth instant open

## 3. Dynamic cooldown, pacing peek, and ranking fixes

- [ ] 3.1 Add `PacingGate::peek_next_wait` (read-only, no permit) in `router/pacing/gate.rs`
- [ ] 3.2 Merge pacing wait into `failure.rs` / `health.rs` cooldown application
- [ ] 3.3 Update `sort.rs` / `rank_score.rs` to use max(slot, model) cooldown in `effective_budget_rank`
- [ ] 3.4 Unit tests: model cooldown deprioritizes; pacing extends cooldown duration; peek is read-only

## 4. Quota snapshot (plan-time headroom)

- [ ] 4.1 Add `router/budget_aware/plan/snapshot.rs` — `QuotaSnapshot::capture()` from pacing registry + health + cooldowns
- [ ] 4.2 Implement `headroom_score(credential_id, model_slug)` using catalog limits from `provider-limits.yaml`
- [ ] 4.3 Wire snapshot capture at start of `plan_route_chain`
- [ ] 4.4 Unit tests: RPD zero → score 0; RPM available → score > 0; catalog slug normalization

## 5. Route chain planner

- [ ] 5.1 Create `router/budget_aware/plan/mod.rs` with `plan_route_chain` and `MAX_PLAN_HOPS = 7`
- [ ] 5.2 Implement filters: circuit-open, zero headroom, zero-success provider, existing intent/payload/ladder
- [ ] 5.3 Implement scoring in `plan/score.rs` per D16: `FEASIBLE(c)`, `score(c)`, fixed v1 weights
- [ ] 5.4 Implement chain build in `plan/build.rs`: memory binding → hash spread → intra-slot ladder UP → cross-provider
- [ ] 5.5 Stability escalation: fast → capacity → stability per `provider-ladders.yaml`; block deprioritized openrouter when Gemini stability has headroom
- [ ] 5.6 Integrate planner in `selection.rs` `ordered_candidates` output
- [ ] 5.7 Update `failover_loop.rs`: walk plan, single replan on exhaustion, record `plan_rebuilds`
- [ ] 5.8 Unit tests in `budget_aware/plan/tests.rs`: hop cap, spread hash, stability order, dead provider excluded, never downgrade on replan

## 6. Work-unit route memory

- [ ] 6.1 Add `router/budget_aware/memory/binding.rs` and `memory/registry.rs` — `WorkUnitRouteMemory` on `moka::future::Cache`
- [ ] 6.2 Planner: prefer viable binding as hop 0; skip when `work_unit_id` absent
- [ ] 6.3 On success: `memory.record(agent, work_unit, binding)`; on failoverable binding failure: `invalidate`
- [ ] 6.4 TTL 30 min via moka `time_to_live`; max capacity 10_000; unit tests in `budget_aware/memory/tests.rs`

## 7. Observability

- [ ] 7.1 Extend `PendingRouteTrace` with `agent_name`, `work_unit_id`, `planned_hops`, `plan_rebuilds`, `route_memory_hit`, `route_memory_invalidated`
- [ ] 7.2 Emit `ReplayRecord` per D19: contract fields, `plan_snapshot_ts`, winner score breakdown, optional top-3 alternatives
- [ ] 7.3 Merge configured credentials into `ProviderRuntimeRegistry::snapshot` with `status: idle`
- [ ] 7.4 Add optional `agent_name` attribute on `AttemptRecord` when context present
- [ ] 7.5 Update `tests/provider_observability.rs` for idle rows, trace fields, and replay record shape

## 8. Routing load scenarios (architectural tests)

- [ ] 8.1 Extend `routing_load` harness with `work_unit_header(id)` and `agent_header(name)` helpers
- [ ] 8.2 Scenario `caller_three_work_units.rs` — 3 concurrent requests, distinct work unit ids
- [ ] 8.3 Scenario `credential_circuit_open.rs` — 429 streak then zero further attempts
- [ ] 8.4 Scenario `route_plan_max_hops.rs` — success with attempts ≤ 7
- [ ] 8.5 Scenario `stability_escalation_plan.rs` — flash-lite before openrouter
- [ ] 8.6 Scenario `stability_never_downgrade.rs` — stability up, zero nemotron attempts
- [ ] 8.7 Scenario `dynamic_cooldown_skip.rs` — pacing skip without HTTP attempt
- [ ] 8.8 Scenario `free_catalog_pacing_skip.rs` — RPD-saturated flash-preview excluded
- [ ] 8.9 Scenario `route_memory_sticky_reuse.rs` — second call reuses binding
- [ ] 8.10 Scenario `route_memory_invalidate_on_429.rs` — 429 clears memory
- [ ] 8.11 Scenario `quota_parallel_collision.rs` — 3 units, 2 headroom keys
- [ ] 8.12 Register all scenarios in `scenarios/mod.rs` and `tests/routing_load.rs`

## 9. Integration and docs

- [ ] 9.1 Run `cargo test --test routing_load --all-features` and targeted unit tests
- [ ] 9.2 Run `cargo clippy` on touched router/metrics modules
- [ ] 9.3 Add `docs/routing.md` section: invoker header contract, stability escalation order, invoker concurrency guidance
- [ ] 9.4 CHANGELOG entry under next beta for caller-context route planning + route memory
- [ ] 9.5 Document invoker driver follow-up: pass `session_id` as work-unit id on structured calls — out of repo

## 10. Invoker driver follow-up (separate repo / PR)

- [ ] 10.1 Gateway driver: pass `session_id` as work-unit id on `analyze_structured` / `chat` calls
- [ ] 10.2 Optional: emit `X-Work-Unit-Id` in gateway driver headers when `session_id` is set
- [ ] 10.3 Document SHOULD limit concurrent LLM calls to healthy free slot estimate when work-unit headers present
