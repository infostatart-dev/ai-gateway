## ADDED Requirements

### Requirement: QuotaSnapshot propagates admission verdict to consumers

The gateway SHALL retain admission verdict fields in `QuotaSnapshot` when
`QuotaSnapshot::capture` evaluates candidates via `evaluate_candidate`, for each
`(credential_id, normalized_model_slug)`:

- `headroom_score` (0.0 when infeasible, 1.0 when feasible)
- `next_wait`
- `blocked_reason` from `AdmissionVerdict`
- `next_available_at` from `AdmissionVerdict`

The gateway MUST consume these fields in plan replay capture and route trace emission.
Write-only capture without downstream readers is non-compliant.

`QuotaSnapshot` SHALL expose accessors `headroom_score`, `next_wait`, `blocked_reason`,
and `next_available_at` keyed by credential and model slug (with catalog normalization).

#### Scenario: Infeasible RPM verdict round-trips through snapshot

- **WHEN** admission returns `feasible: false`, `blocked_reason: rpm`, `next_wait > 0`
- **AND** `QuotaSnapshot::capture` includes that candidate
- **THEN** `snapshot.blocked_reason(credential, model)` is `rpm`
- **AND** `snapshot.next_available_at(credential, model)` equals the verdict instant
- **AND** `snapshot.headroom_score(credential, model)` is `0.0`

#### Scenario: Snapshot accessors used by replay capture

- **WHEN** `capture_replay` builds `PlanReplaySnapshot`
- **THEN** it calls `QuotaSnapshot::blocked_reason` and `QuotaSnapshot::next_available_at`
  for quota-excluded pool members
- **AND** rustc emits no `dead_code` warning on `QuotaSnapshotEntry.blocked_reason`

#### Scenario: Feasible candidate snapshot

- **WHEN** admission returns `feasible: true` and `blocked_reason: none`
- **THEN** `headroom_score` is `1.0`
- **AND** `blocked_reason` accessor returns `none`
