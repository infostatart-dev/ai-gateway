# routing-observability

## ADDED Requirements

### Requirement: Credential routing health in provider-stats

`GET /v1/observability/provider-stats` SHALL include a `routing_health` object on each
provider row with:

- `circuit_open` (bool)
- `open_until` (RFC3339 timestamp, present when `circuit_open` is true)
- `success_rate` (float 0.0–1.0, rolling window)
- `planner_excluded` (bool) — true when circuit is open or credential is dead in the
  health registry

Values SHALL reflect `CredentialHealthRegistry` state at snapshot time.

#### Scenario: Circuit-open credential visible in stats

- **WHEN** `gemini-free-8` has circuit-open in the health registry
- **AND** provider-stats is queried
- **THEN** the row for `gemini-free-8` includes `routing_health.circuit_open = true`
- **AND** includes `routing_health.planner_excluded = true`

#### Scenario: Healthy credential not excluded

- **WHEN** `gemini-free-9` has no open circuit and success rate above threshold
- **THEN** `routing_health.planner_excluded` is false

#### Scenario: Idle configured credential includes health defaults

- **WHEN** a configured credential has zero attempts since startup
- **THEN** the idle row still includes `routing_health` with `circuit_open = false`
- **AND** `success_rate = 1.0` (no failures observed)

### Requirement: Quota capacity terminology in observability

Operator-facing JSON and structured route logs SHALL use **quota capacity** naming:

- Replay score field: `quota_capacity` (0.0–1.0 at plan time)
- Route trace and docs SHALL refer to "quota capacity at plan time", not "headroom"

The gateway MAY emit deprecated alias `q_headroom` with the same numeric value for one
minor release.

#### Scenario: Replay record uses quota capacity field

- **WHEN** a routed request emits `ReplayRecord`
- **THEN** the winner score breakdown includes `quota_capacity`
- **AND** the value equals the planner quota capacity score for that hop

#### Scenario: Deprecated alias preserved

- **WHEN** a consumer reads `q_headroom` from replay JSON
- **THEN** it receives the same value as `quota_capacity` during the deprecation window

## MODIFIED Requirements

### Requirement: Replayable routing decision log

The gateway MUST emit a `ReplayRecord` in the per-request route trace (structured log)
sufficient to reconstruct the operational routing decision without message semantics.

The record SHALL include at minimum:

- Request contract: `source_model`, `json_schema_required`, `agent_name`, `work_unit_id`,
  `work_unit_source`
- `plan_snapshot_ts` (instant or monotonic counter at plan time)
- Plan metadata: `planned_hops`, `plan_rebuilds`, `route_memory_hit`,
  `route_memory_invalidated`
- Winner hop 0: `credential_id`, `model_slug`, aggregate `score`
- Score breakdown for winner: `h_success`, `quota_capacity`, `q_cooldown_secs`,
  `m_affinity`, `hash_bias`, `l_band`, `cost_class`

The record SHOULD include up to three next-best feasible alternatives with
`credential_id`, `model_slug`, and `score`.

Replay tooling is not required in v1; the log contract MUST be stable for offline
incident analysis.

#### Scenario: Trace supports incident replay

- **WHEN** a routed request completes (success or terminal failure)
- **THEN** structured route trace contains winner hop 0 score breakdown with
  `quota_capacity`
- **AND** `plan_snapshot_ts` is present
- **AND** no message body or prompt text is required to explain the hop-0 choice

#### Scenario: Replan emits second snapshot

- **WHEN** the router performs a plan rebuild after initial plan exhaustion
- **THEN** route trace records `plan_rebuilds=1`
- **AND** the terminal trace references the snapshot used for the successful or final hop

### Requirement: Route trace plan and memory metadata

The per-request routing trace SHALL include `planned_hops`, `plan_rebuilds`,
`agent_name`, `work_unit_id`, `work_unit_source`, `route_memory_hit`, and
`route_memory_invalidated` in structured log emission.

#### Scenario: Plan metadata on multi-hop success

- **WHEN** a request succeeds on hop 3 of a 5-hop plan
- **THEN** trace reports `planned_hops=5`, `upstream_attempts=3`, `plan_rebuilds=0`

#### Scenario: Memory hit in trace

- **WHEN** a work unit reuses a remembered binding
- **THEN** trace reports `route_memory_hit=true`
