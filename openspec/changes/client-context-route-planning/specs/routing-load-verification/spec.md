# routing-load-verification

## ADDED Requirements

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

#### Scenario: Contributor adds caller-context scenario

- **WHEN** a contributor adds `routing_load/scenarios/caller_three_work_units.rs`
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
