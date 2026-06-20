# routing-load-verification Specification

## Purpose
TBD - created by archiving change client-context-route-planning. Update Purpose after archive.
## Requirements
### Requirement: Caller context routing load scenarios

The routing load verification catalog SHALL include scenarios that prove
`caller-request-context`, `credential-health-routing`, `route-chain-planning`,
`work-unit-route-memory`, and `quota-headroom-scheduling` without live API keys.

Each scenario SHALL assert outcomes via `GET /v1/observability/provider-stats` and
route trace fields, not LLM response quality.

Scenarios SHALL follow the shared harness pattern:

```text
RoutingLoadHarness → mocked upstream responses → autodefault HTTP or
budget_aware test entry → assert_stats / assert_identity / route trace
```

#### Scenario: Caller-context catalog proves planner without live keys

- **WHEN** the routing_load catalog runs caller-context scenarios
- **THEN** each scenario uses declarative `UpstreamMockScript` mocks
- **AND** outcomes are asserted via provider-stats or route trace fields, not LLM quality

### Requirement: Full routing-load test catalog layout

The repository SHALL organize routing verification tests as follows (D8). Production
code MUST NOT embed `#[cfg(test)]` modules for planner/memory/routing_load.

| Binary / crate | Path | Role |
|----------------|------|------|
| Integration catalog | `ai-gateway/tests/routing_load.rs` | Registers `#[serial_test::serial]` scenario runners |
| Scenario modules | `ai-gateway/tests/rl/scenarios/*.rs` | One `pub async fn run()` per file |
| Scenario prelude | `ai-gateway/tests/rl/support.rs` | Re-exports `routing_harness` helpers |
| Scenario helpers | `ai-gateway/tests/rl/helpers.rs` | Shared warm-up (`trip_circuit`, …) |
| Harness (feature `testing`) | `ai-gateway/src/tests/routing_harness/` | `run_planned_failover`, assert_stats, pacing |
| Public test facade | `ai-gateway/src/tests/routing.rs` | Stable imports for integration tests |
| Shared upstream mocks | `crates/gateway-tests/src/upstream/` | `UpstreamMockScript`, response factories |
| Unit: planner | `ai-gateway/tests/budget_aware_plan.rs` | `MAX_PLAN_HOPS`, dead provider, spread |
| Unit: memory | `ai-gateway/tests/budget_aware_memory.rs` | sticky binding, TTL |
| Unit: snapshot | `ai-gateway/tests/budget_aware_snapshot.rs` | RPD/RPM headroom scoring |
| Unit: health | `ai-gateway/tests/credential_health_registry.rs` | circuit open/close |
| Unit: caller | `ai-gateway/tests/caller_context.rs` | header precedence |
| Unit: observability | `ai-gateway/tests/provider_observability.rs` | provider-stats + usage header |

#### Scenario: Client-context layer (declarative mocks required)

| File | Proves |
|------|--------|
| `caller_three_work_units.rs` | 3 work units spread healthy credentials; dead circuits untouched |
| `credential_circuit_open.rs` | circuit-open slot receives zero follow-up attempts |
| `route_plan_max_hops.rs` | terminal success with `planned_hops` ≤ 7 |
| `stability_escalation_plan.rs` | capacity/stability gemini before openrouter |
| `stability_never_downgrade.rs` | `gemini-2.5-flash-lite` wins; zero nemotron attempts |
| `dynamic_cooldown_skip.rs` | RPM-saturated preview skipped (404 trap, no HTTP) |
| `free_catalog_pacing_skip.rs` | RPD-saturated `gemini-3-flash-preview` excluded at plan |
| `route_memory_sticky_reuse.rs` | 2nd call `route_memory_hit=true`, same binding |
| `route_memory_invalidate_on_429.rs` | 429 on binding → failover hop |
| `quota_parallel_collision.rs` | saturated slots excluded; routes stay on headroom keys |

#### Scenario: Legacy load / failover catalog (FIFO migration in progress)

| File | Proves |
|------|--------|
| `intent_fast_thinking_pool.rs` | intent-tier pool selection |
| `round_robin.rs` | concurrent spread across slots |
| `gemini_sixteen_slot.rs` | 16-slot fairness band |
| `gemini_model_ladder_same_slot.rs` | intra-slot model ladder |
| `gemini_stability_escalation.rs` | legacy walk stability band |
| `gemini_stability_escalates_up.rs` | fast exhausted → flash-lite |
| `gemini_404_retires_model_not_slot.rs` | 404 retires model only |
| `gemini_503_high_demand_continues_ladder.rs` | 503 continues ladder |
| `payload_filter.rs` | payload estimate filter under load |
| `failover_rpm.rs` | RPM failover sibling |
| `failover_quota.rs` / `failover_daily_quota.rs` | quota / daily pacing failover |
| `chatgpt_last_resort.rs` | chatgpt-web last resort |
| `openrouter_nemotron_429_then_gpt_oss_200.rs` | openrouter model failover |
| `openrouter_402_paid_does_not_kill_free.rs` | 402 isolation |
| `pacing_burst.rs` | pacing gate backpressure |
| `deepseek_*` (3 files) | deepseek restriction failover |
| `shaper_backpressure.rs` | shaper limits |
| HTTP harness smoke (`harness_round_robin`, `harness_payload_filter`) | HTTP harness smoke; MUST use `source_model_selection: Intent` (autodefault parity) and `x-work-unit-id` when asserting credential spread |

Legacy scenarios MAY still use `push_test_call_response*` until task 10.5 migration;
new client-context scenarios MUST use `UpstreamMockScript` only.

All new routing load scenarios and router integration tests SHALL use the workspace
crate `gateway-tests` with declarative `UpstreamMockScript` matching upstream hops by
`(credential_id, model_slug)` and per-hop attempt index.

Legacy `push_test_call_response` / `push_test_call_response_for_credential` FIFO queues
are **deprecated** for new tests. They MAY remain only during migration of existing
scenarios; contributors MUST NOT add new FIFO-only multi-push scripts.

Each workspace library crate (`ai-gateway`, `chatgpt-web`, `deepseek-web`, …) SHALL
list `gateway-tests` as a `dev-dependency` for shared fixtures.

#### Scenario: Binding hop returns scripted status

- **WHEN** a scenario installs
  `UpstreamMockScript::binding("gemini-free-9", "gemini-3.1-flash-lite", [ok, 429])`
- **AND** the router walks that binding twice in one inbound request
- **THEN** the first upstream pop returns 200 and the second returns 429
- **AND** no additional `push_*` calls are required for that binding

#### Scenario: Contributor adds caller-context scenario

- **WHEN** a contributor adds `tests/rl/scenarios/caller_three_work_units.rs`
- **THEN** the scenario reuses `RoutingLoadHarness` and stats assertion helpers
- **AND** registers in `scenarios/mod.rs` and `tests/routing_load.rs`

### Requirement: Three work units spread healthy credentials

Scenario `caller_three_work_units` SHALL send three concurrent autodefault requests
with distinct work-unit headers (`X-Work-Unit-Id` or `Helicone-Session-Id`) and the
same invoker name.

Given mocked responses where `gemini-free-2`..`gemini-free-8` always return 429 and
`gemini-free-9`, `gemini-free-10`, `openrouter-default` return 200, the scenario
SHALL assert:

- zero provider-stats attempts on circuit-open credentials after warm-up phase
- first-attempt credential ids differ across the three work units when ≥3 healthy
  credentials exist

#### Scenario: Three work units three keys

- **WHEN** the three-work-units scenario completes
- **THEN** provider-stats shows successful attempts on at least two distinct healthy
  gemini credentials across the three inbound requests
- **AND** `gemini-free-2` attempt count does not increase after circuits open

### Requirement: Credential circuit-open scenario

Scenario `credential_circuit_open` SHALL inject a streak of ≥5 failoverable failures
on one credential, then send additional requests.

#### Scenario: Circuit stops attempts

- **WHEN** warm-up exhausts `gemini-free-8` with repeated 429
- **AND** a follow-up request is sent with a new work unit id
- **THEN** provider-stats shows no further attempts on `gemini-free-8` until circuit TTL

### Requirement: Route plan hop cap scenario

Scenario `route_plan_max_hops` SHALL use mixed failure mocks across providers and
assert terminal success with `upstream_attempts` ≤ 7 on the successful inbound
request per route trace or `X-Gateway-Provider-Usage` routing block.

#### Scenario: Success within hop budget

- **WHEN** the hop-cap scenario completes successfully
- **THEN** at least one inbound request reports `upstream_attempts` less than or equal to 7

### Requirement: Stability escalation plan scenario

Scenario `stability_escalation_plan` SHALL exhaust fast-band Gemini models on a slot
via mocks (429 or daily quota) while capacity/stability models on the same slot return 200.

#### Scenario: Flash-lite before cross-provider

- **WHEN** the stability scenario completes
- **THEN** route trace or stats show a successful attempt on a stability-band or
  capacity-band model slug before any openrouter attempt for the same inbound request

### Requirement: Stability never downgrade scenario

Scenario `stability_never_downgrade` SHALL prove client-ordered stability:

- Fast-band models return 429 on preferred Gemini slot
- `gemini-2.5-flash-lite` on same slot returns 200
- Openrouter nemotron (deprioritized) is configured to return 200 if attempted

#### Scenario: Stability up not nemotron

- **WHEN** the never-downgrade scenario completes
- **THEN** routed identity includes `gemini-2.5-flash-lite`
- **AND** provider-stats shows zero attempts on nemotron for that inbound request

### Requirement: Dynamic cooldown skip scenario

Scenario `dynamic_cooldown_skip` SHALL saturate per-model pacing for one model slug
without HTTP upstream call on the saturated model during the plan window.

#### Scenario: Pacing skip avoids HTTP

- **WHEN** pacing gate reports no available slot for `gemini-3-flash-preview` on a credential
- **THEN** the planner skips that candidate without incrementing provider-stats attempts
  for that model during the skip window

### Requirement: Free catalog pacing skip scenario

Scenario `free_catalog_pacing_skip` SHALL pre-saturate RPD for a low-RPD catalog model
(`gemini-3-flash-preview`, RPD 20 per embedded limits) via pacing harness and send a
request that would previously try that model first.

#### Scenario: Catalog RPD excludes model from plan

- **WHEN** RPD is exhausted for `gemini-3-flash-preview` on a slot
- **AND** `gemini-3.1-flash-lite` on the same slot has headroom
- **THEN** the first upstream attempt is not `gemini-3-flash-preview`
- **AND** routed identity uses flash-lite

### Requirement: Route memory sticky reuse scenario

Scenario `route_memory_sticky_reuse` SHALL:

1. Send request A with work unit `unit-47` → success on `gemini-free-9/gemini-3.1-flash-lite`
2. Send request B with same agent and work unit → assert first hop reuses binding
3. Assert `route_memory_hit=true` on request B

#### Scenario: Second call hits memory

- **WHEN** sticky reuse scenario completes request B
- **THEN** first upstream attempt targets the same credential and model as request A
- **AND** route trace reports `route_memory_hit=true`

### Requirement: Route memory invalidate scenario

Scenario `route_memory_invalidate_on_429` SHALL:

1. Establish binding via success (as sticky scenario)
2. Mock next call to return 429 on binding
3. Send request C with same work unit → assert binding invalidated and new first hop

#### Scenario: 429 clears sticky route

- **WHEN** request C is processed after 429 on binding
- **THEN** route trace reports `route_memory_invalidated=true`
- **AND** first hop differs from the invalidated binding when alternatives have headroom

### Requirement: Quota parallel collision scenario

Scenario `quota_parallel_collision` SHALL simulate three concurrent requests where only
two Gemini credentials have RPM headroom for the preferred model at plan instant.

#### Scenario: Third work unit avoids saturated pair

- **WHEN** three concurrent requests plan with work units `unit-1`, `unit-2`, `unit-3`
- **AND** only `gemini-free-9` and `gemini-free-10` have headroom
- **THEN** at most two requests use those credentials as first hop
- **AND** the third request's first hop uses a different provider or credential with headroom

### Requirement: Admission control routing load catalog

The routing load verification catalog SHALL include scenarios proving `quota-admission-control`:

| File | Proves |
|------|--------|
| `admission_zero_repeat_429.rs` | After reconcile blocks scope, no second upstream 429 on same scope |
| `admission_parallel_account_spread.rs` | N work units use N distinct feasible accounts (no pool cap) |
| `admission_hop_readmit.rs` | Re-admit after first 429 routes to sibling without repeat 429 |
| `admission_longcat_tpd.rs` | LongCat TPD from catalog drives infeasible without magic constants |
| `admission_per_session_deepseek.rs` | `deepseek-web-2` session scope admits independently of sibling |

Each scenario SHALL assert `repeat_429_violation` is absent and attempt counts match expected HTTP.

#### Scenario: admission_zero_repeat_429

- **WHEN** first request triggers reconcile block on a `CredentialModel` scope
- **THEN** second concurrent request does not HTTP to that scope
- **AND** `repeat_429_violations` remains 0

#### Scenario: admission_parallel_account_spread with sixteen Gemini secrets

- **WHEN** sixteen `gemini-free*` credentials are configured and feasible
- **AND** eight concurrent work units arrive
- **THEN** at least eight distinct accounts receive first-hop attempts when all are feasible

#### Scenario: admission_per_session_deepseek

- **WHEN** `deepseek-web-default` is infeasible (session cooldown)
- **AND** `deepseek-web-2` is feasible
- **THEN** traffic uses `deepseek-web-2` without inheriting sibling block

### Requirement: Unit tests cover three quota profiles

`ai-gateway/tests/quota_admission.rs` SHALL test admission verdicts for `per-model`, `per-slot`, and
`per-session` scopes using catalog fixtures without live API keys.

#### Scenario: Three-profile matrix passes in CI

- **WHEN** `cargo test quota_admission` runs with all features
- **THEN** per-model, per-slot, and per-session admission cases pass

