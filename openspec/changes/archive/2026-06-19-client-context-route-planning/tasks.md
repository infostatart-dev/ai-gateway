## 1. Caller request context

- [x] 1.1 Add `CallerRequestContext` to `types/extensions.rs` (`agent_name`, `work_unit_id`)
- [x] 1.2 Implement `middleware/caller_context/` — parse `X-Agent-Name`, `Helicone-Property-Agent`, `X-Work-Unit-Id`, `Helicone-Session-Id`
- [x] 1.3 Wire `CallerContextLayer` into router stack in `app/stack.rs` or `router/meta.rs`
- [x] 1.4 Unit tests: header precedence and missing defaults (`tests/caller_context.rs`)

## 2. Credential health registry

- [x] 2.1 Add `CredentialHealthRegistry` in `router/budget_aware/health_registry.rs` (rolling window, circuit state)
- [x] 2.2 Hook health updates from `metrics/provider/runtime.rs` `record_attempt`
- [x] 2.3 Implement circuit-open rules (window min 5, success <10%, 401 immediate, slot-all-models cooldown)
- [x] 2.4 Expose `is_circuit_open`, `success_rate`, `provider_zero_success` queries for planner
- [x] 2.5 Unit tests: circuit open/close, window rollover, auth instant open (`tests/credential_health_registry.rs`)

## 3. Dynamic cooldown, pacing peek, and ranking fixes

- [x] 3.1 Add `PacingGate::peek_next_wait` (read-only, no permit) in `router/pacing/gate.rs`
- [x] 3.2 Merge pacing wait into `failure.rs` / `health.rs` cooldown application
- [x] 3.3 Update `sort.rs` / `rank_score.rs` to use max(slot, model) cooldown in `effective_budget_rank`
- [x] 3.4 Unit tests: model cooldown deprioritizes; pacing extends cooldown duration; peek is read-only (`plain_chat_widens_intent_floor` in `budget_aware_plan.rs`)

## 4. Quota snapshot (plan-time headroom)

- [x] 4.1 Add `router/budget_aware/plan/snapshot.rs` — `QuotaSnapshot::capture()` from pacing registry + health + cooldowns
- [x] 4.2 Implement `headroom_score(credential_id, model_slug)` using catalog limits from `provider-limits.yaml`
- [x] 4.3 Wire snapshot capture at start of `plan_route_chain`
- [x] 4.4 Unit tests: RPD zero → score 0; RPM available → score > 0; catalog slug normalization (`tests/budget_aware_snapshot.rs`)

## 5. Route chain planner

- [x] 5.1 Create `router/budget_aware/plan/mod.rs` with `plan_route_chain` and `MAX_PLAN_HOPS = 7`
- [x] 5.2 Implement filters: circuit-open, zero headroom, zero-success provider, existing intent/payload/ladder
- [x] 5.3 Implement scoring in `plan/score.rs` per D16: `FEASIBLE(c)`, `score(c)`, fixed v1 weights
- [x] 5.4 Implement chain build in `plan/build.rs`: memory binding → hash spread → intra-slot ladder UP → cross-provider
- [x] 5.5 Stability escalation: fast → capacity → stability per `provider-ladders.yaml`; block deprioritized openrouter when Gemini stability has headroom
- [x] 5.6 Integrate planner in `selection.rs` `ordered_candidates` output
- [x] 5.7 Update `failover_loop.rs`: walk plan, single replan on exhaustion, record `plan_rebuilds`
- [x] 5.8 Unit tests in `tests/budget_aware_plan.rs`: hop cap, spread hash, stability order, dead provider excluded, never downgrade on replan

## 6. Work-unit route memory

- [x] 6.1 Add `router/budget_aware/memory/binding.rs` and `memory/registry.rs` — `WorkUnitRouteMemory` on `moka::future::Cache`
- [x] 6.2 Planner: prefer viable binding as hop 0; skip when `work_unit_id` absent
- [x] 6.3 On success: `memory.record(agent, work_unit, binding)`; on failoverable binding failure: `invalidate`
- [x] 6.4 TTL 30 min via moka `time_to_live`; max capacity 10_000; unit tests in `tests/budget_aware_memory.rs`

## 7. Observability

- [x] 7.1 Extend `PendingRouteTrace` with `agent_name`, `work_unit_id`, `planned_hops`, `plan_rebuilds`, `route_memory_hit`, `route_memory_invalidated`
- [x] 7.2 Emit `ReplayRecord` per D19: contract fields, `plan_snapshot_ts`, winner score breakdown, optional top-3 alternatives
- [x] 7.3 Merge configured credentials into `ProviderRuntimeRegistry::snapshot` with `status: idle`
- [x] 7.4 Add optional `agent_name` attribute on `AttemptRecord` when context present
- [x] 7.5 Update `tests/provider_observability.rs` for idle rows, trace fields, and replay record shape

## 8. Routing load scenarios (architectural tests)

- [x] 8.1 Extend `routing_load` harness with `work_unit_header(id)` and `agent_header(name)` helpers (`src/tests/routing_harness/headers.rs`)
- [x] 8.2 Scenario `tests/rl/scenarios/caller_three_work_units.rs` — 3 concurrent requests, distinct work unit ids
- [x] 8.3 Scenario `tests/rl/scenarios/credential_circuit_open.rs` — 429 streak then zero further attempts
- [x] 8.4 Scenario `tests/rl/scenarios/route_plan_max_hops.rs` — success with `planned_hops` ≤ 7
- [x] 8.5 Scenario `tests/rl/scenarios/stability_escalation_plan.rs` — flash-lite before openrouter
- [x] 8.6 Scenario `tests/rl/scenarios/stability_never_downgrade.rs` — stability up, zero nemotron attempts
- [x] 8.7 Scenario `tests/rl/scenarios/dynamic_cooldown_skip.rs` — pacing skip without HTTP attempt
- [x] 8.8 Scenario `tests/rl/scenarios/free_catalog_pacing_skip.rs` — RPD-saturated flash-preview excluded
- [x] 8.9 Scenario `tests/rl/scenarios/route_memory_sticky_reuse.rs` — second call reuses binding
- [x] 8.10 Scenario `tests/rl/scenarios/route_memory_invalidate_on_429.rs` — 429 clears memory
- [x] 8.11 Scenario `tests/rl/scenarios/quota_parallel_collision.rs` — saturated slots excluded; headroom-only routes
- [x] 8.12 Register all scenarios in `tests/rl/scenarios/mod.rs` and `tests/routing_load.rs`

## 9. Integration and docs

- [x] 9.1 Run `cargo test --test routing_load --features testing` and targeted unit tests (client-context slice)
- [x] 9.2 Run `cargo clippy` on touched router/metrics modules
- [x] 9.3 Add `docs/routing.md` section: invoker header contract, stability escalation order, invoker concurrency guidance
- [x] 9.4 CHANGELOG entry under next beta for caller-context route planning + route memory
- [x] 9.5 Document invoker driver follow-up: pass `session_id` as work-unit id on structured calls — out of repo

## 10. Industrial test harness (`gateway-tests`)

- [x] 10.1 Add workspace crate `crates/gateway-tests` with `UpstreamMockScript` (binding/credential/default)
- [x] 10.2 Wire `budget_aware/call.rs` to `pop(credential, model)` — script before legacy FIFO
- [x] 10.3 Document industrial test rules in `design.md` D8 and `routing-load-verification` spec
- [x] 10.4 Add `gateway-tests` dev-dependency to every workspace library crate
- [x] 10.5 Migrate `routing_load` scenarios off FIFO-only scripts (all 32 scenarios declarative)
- [x] 10.6 Rewrite `route_memory_invalidate_on_429` with declarative script + binding invalidate fix

## 11. Invoker driver follow-up (separate repo / PR)

- [x] 11.1 Gateway driver: pass `session_id` as work-unit id on `analyze_structured` / `chat` calls — **deferred:** contract in [docs/invoker-driver-follow-up.md](../../docs/invoker-driver-follow-up.md); code in invoker repo
- [x] 11.2 Optional: emit `X-Work-Unit-Id` in gateway driver headers when `session_id` is set — **deferred:** same contract doc; implement with 11.1 in invoker repo
- [x] 11.3 Document SHOULD limit concurrent LLM calls to healthy free slot estimate when work-unit headers present (`docs/routing.md`)
