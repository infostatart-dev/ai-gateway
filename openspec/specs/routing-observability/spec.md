# routing-observability

## Purpose

Attribute router failover, cooldown, and quota outcomes to credential slots and
limit dimensions, and emit a single per-request routing trace summary for
multi-account autodefault debugging.
## Requirements
### Requirement: Per-credential failover and cooldown attribution
Router failover and cooldown metrics SHALL carry a `credential` attribute
identifying the upstream account slot, in addition to the existing provider
attribution, so multi-account behavior (e.g. four Gemini free slots) is
distinguishable without log scraping.

#### Scenario: Failover metric distinguishes free slots
- **WHEN** the router fails over from `gemini-free` to `gemini-free-2`
- **THEN** the failover metric records the originating `credential`
- **AND** the value is distinct from a failover originating on `gemini-default`

### Requirement: Quota-metric attribution on rate-limit outcomes
The router SHALL annotate rate-limit, quota, and overload outcome metrics with a
`quota_metric` attribute describing which limit was hit, using one of `rpm`,
`tpm`, `rpd`, `context`, or `overload`.

#### Scenario: Per-minute token cap failure is labeled tpm
- **WHEN** a candidate returns a per-minute token-cap error (e.g. groq 413 TPM)
- **THEN** the metric is annotated with `quota_metric = tpm`

#### Scenario: Daily quota exhaustion is labeled rpd
- **WHEN** a candidate returns a daily quota-exhausted error
- **THEN** the metric is annotated with `quota_metric = rpd`

#### Scenario: Overload is labeled overload
- **WHEN** a candidate returns a `503` overload response
- **THEN** the metric is annotated with `quota_metric = overload`

### Requirement: Per-request routing trace summary
At the end of a router request, the router SHALL emit one structured summary
event capturing at least: number of upstream hops, total wall-clock duration in
milliseconds, the terminal provider and credential, the terminal status, and
counts of candidates skipped pre-flight by payload-aware filtering.

#### Scenario: Summary emitted on success
- **WHEN** a request completes successfully after several failovers
- **THEN** a single summary event reports hop count, duration, terminal provider/credential, and skipped-candidate counts

#### Scenario: Summary emitted on terminal failure
- **WHEN** a request exhausts all candidates without success
- **THEN** a single summary event reports the same fields with the terminal failure status

### Requirement: Terminal routing summary includes generation efficiency

At the end of a router request, the structured routing trace summary SHALL include
`generation_ms_per_output_token` (nullable), `upstream_attempts`, and `terminal_outcome`
when an upstream attempt was made.

#### Scenario: Summary after failover success

- **WHEN** a request fails once then succeeds with terminal `output_tokens=20`, duration
  `800 ms`, and `tfft_ms=200`
- **THEN** the summary reports `upstream_attempts=2`
- **AND** `terminal_outcome=success`
- **AND** `generation_ms_per_output_token=30.0`

#### Scenario: Summary on terminal failure

- **WHEN** all upstream attempts fail without a successful body
- **THEN** the summary reports `upstream_attempts` equal to the number of attempts
- **AND** `terminal_outcome` reflects the last failure class
- **AND** `generation_ms_per_output_token` is null

### Requirement: Configured credential inventory in provider-stats

`GET /v1/observability/provider-stats` SHALL return one row per configured
credential in `credentials.yaml`, including credentials with zero attempts since
process start.

Idle rows SHALL report `calls.attempts = 0` and SHALL include `status: idle` in the
JSON representation.

#### Scenario: ChatGPT configured but unused appears idle

- **WHEN** `chatgpt-web-default` is configured
- **AND** no upstream attempt referenced that credential since startup
- **THEN** provider-stats includes a row for `(chatgpt-web, chatgpt-web-default)`
- **AND** attempts equal zero
- **AND** status is `idle`

#### Scenario: Active credential shows runtime stats

- **WHEN** `gemini-free-9` has recorded attempts
- **THEN** the same row includes non-zero attempt counters and omits idle status

### Requirement: Invoker attribution on upstream attempts

When `CallerRequestContext` is present, the gateway MUST record `agent_name` on
upstream attempt metrics and MUST include `agent_name` in the terminal route
trace summary.

The top-level provider-stats list SHALL remain per `(provider, credential)` in v1.

#### Scenario: Route trace carries invoker name

- **WHEN** a request includes `X-Agent-Name: invoker-alpha`
- **AND** an upstream attempt is recorded
- **THEN** the terminal route trace includes `agent_name = invoker-alpha`

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

### Requirement: Provider-stats exposes hierarchical quota tree

`GET /v1/observability/provider-stats` SHALL return quota observability as a tree aligned with
admission hierarchy:

```text
quota[] → { provider, accounts[] } → models[] (when quota-profile: per-model)
```

Each **account** node SHALL include: `credential_id`, `quota_profile`, `calls`, routing health,
`next_available_at`, `blocked_reason`.

Each **model** node (per-model providers only) SHALL include: `slug`, `next_available_at`,
`blocked_reason`, attempt counters when non-zero.

When limits apply only at account level (`per-slot`, `per-session`), model nodes SHALL be omitted
and limits are understood to inherit from the account node.

The flat `providers[]` array SHALL remain for backward-compatible call counters; enriched rows MAY
duplicate `quota_profile`, `next_available_at`, and `blocked_reason` from the tree.

#### Scenario: Gemini account shows per-model children

- **WHEN** provider `gemini` has `quota-profile: per-model`
- **AND** `gemini-free-3` has blocked preview slug and feasible flash-lite slug
- **THEN** account row includes `models[]` with distinct `next_available_at` per slug

#### Scenario: LongCat account has no model children

- **WHEN** provider `longcat` is per-slot
- **THEN** account row has no `models[]`
- **AND** `next_available_at` on the account reflects shared gate state

### Requirement: Repeat 429 violations on observability snapshot

The provider-stats snapshot root SHALL include `repeat_429_violations` (count since process start).
The gateway SHALL expose the same counter as OpenTelemetry metric
`gateway_repeat_429_violations_total`. Route trace hops SHALL include `repeat_429_violation` when
applicable.

#### Scenario: Clean deployment shows zero violations

- **WHEN** no infeasible scope receives upstream 429
- **THEN** `repeat_429_violations` is 0

