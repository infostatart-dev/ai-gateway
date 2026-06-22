## MODIFIED Requirements

### Requirement: Replayable routing decision log

The gateway MUST emit a `ReplayRecord` in the per-request route trace (structured log)
sufficient to reconstruct the operational routing decision without message semantics.

The record SHALL include at minimum:

- Request contract: `source_model`, `json_schema_required`, `agent_name`, `work_unit_id`,
  `work_unit_source`
- `plan_snapshot_ts` (instant or monotonic counter at plan time)
- Plan metadata: `planned_hops`, `plan_rebuilds`, `route_memory_hit`, `route_memory_invalidated`
- Winner hop 0: `credential_id`, `model_slug`, aggregate `score`
- Score breakdown for winner: `h_success`, `quota_capacity`, `q_cooldown_secs`, `m_affinity`,
  `hash_bias`, `l_band`, `cost_class`
- When winner `quota_capacity` is `0.0` at plan time: `blocked_reason` and
  `next_available_at` on the winner score breakdown (from `QuotaSnapshot`, not live
  re-evaluation)

The record SHOULD include up to three next-best feasible alternatives with
`credential_id`, `model_slug`, and `score`.

The record SHOULD include up to eight `quota_excluded` entries for candidates omitted
from the plan because `QuotaSnapshot.headroom_score == 0.0`, each with `credential`,
`model`, `blocked_reason`, `next_available_at`, and `quota_capacity: 0.0`.

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

#### Scenario: Quota-excluded candidate visible in replay

- **GIVEN** `gemini-3-flash-preview` on `gemini-free-3` has `QuotaSnapshot.headroom_score == 0.0`
- **AND** `blocked_reason` is `rpm` at plan time
- **WHEN** the plan selects a different feasible hop as winner
- **THEN** `ReplayRecord.quota_excluded` includes an entry for `gemini-free-3` +
  `gemini-3-flash-preview` with `blocked_reason: rpm`
- **AND** `next_available_at` matches the plan-time snapshot

#### Scenario: Feasible winner omits block metadata

- **WHEN** the winning hop has `quota_capacity > 0.0` at plan time
- **THEN** winner score breakdown omits `blocked_reason` and `next_available_at`

## ADDED Requirements

### Requirement: Plan-time quota metadata matches provider-stats vocabulary

Replay and route trace quota block fields SHALL use the same `blocked_reason` enum
values and RFC3339 `next_available_at` format as `GET /v1/observability/provider-stats`
account and model nodes.

Values MUST be copied from `QuotaSnapshot` captured during `plan_route_chain`, not from
a second live admission evaluation at trace emit time.

#### Scenario: Replay blocked reason matches snapshot

- **WHEN** `QuotaSnapshot.blocked_reason(credential, model)` returns `tpd`
- **AND** that pair appears in `quota_excluded`
- **THEN** the replay entry `blocked_reason` is `tpd`
