## ADDED Requirements

### Requirement: Plan-time quota exclusion trace references snapshot block reason

The plan replay payload SHALL record quota exclusions when route planning omits a
candidate because `QuotaSnapshot.headroom_score == 0.0`, including `blocked_reason` and
`next_available_at` from the same snapshot used for plan construction.

Health-driven exclusions (circuit-open, zero-success dead provider) SHALL NOT appear
in `quota_excluded`; those remain visible via `routing_health.planner_excluded` on
provider-stats.

#### Scenario: Preview slug excluded with RPM reason

- **GIVEN** `gemini-3-flash-preview` on `gemini-free-3` is infeasible at plan time due to RPM
- **WHEN** `plan_route_chain` builds a plan without that candidate
- **THEN** plan replay includes `quota_excluded` entry with `blocked_reason: rpm`
- **AND** the planned chain does not contain that candidate

#### Scenario: Circuit-open credential not in quota_excluded

- **GIVEN** `gemini-free-8` is circuit-open
- **WHEN** plan construction excludes it
- **THEN** `quota_excluded` does not list `gemini-free-8` solely for circuit-open
- **AND** provider-stats shows `routing_health.planner_excluded = true`

## MODIFIED Requirements

### Requirement: Plan observability fields

The route trace SHALL record `planned_hops` (plan length before walk),
`plan_rebuilds` (count of replan invocations), `route_memory_hit`, and
`route_memory_invalidated`.

When plan replay is present, the trace SHALL include quota exclusion metadata via
`ReplayRecord.quota_excluded` propagated from `PlanReplaySnapshot`.

#### Scenario: Trace includes plan metadata

- **WHEN** a request plans 5 hops and succeeds on hop 2
- **THEN** route trace reports `planned_hops=5` and `upstream_attempts=2`

#### Scenario: Trace includes quota exclusions from plan snapshot

- **WHEN** two pool candidates are quota-infeasible at plan time
- **AND** the plan selects a feasible winner
- **THEN** route trace replay includes up to eight `quota_excluded` entries with
  plan-time `blocked_reason`
