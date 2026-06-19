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
`agent_name`, `work_unit_id` (nullable), `route_memory_hit`, and
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

- Request contract: `source_model`, `json_schema_required`, `agent_name`, `work_unit_id`
- `plan_snapshot_ts` (instant or monotonic counter at plan time)
- Plan metadata: `planned_hops`, `plan_rebuilds`, `route_memory_hit`, `route_memory_invalidated`
- Winner hop 0: `credential_id`, `model_slug`, aggregate `score`
- Score breakdown for winner: `h_success`, `q_headroom`, `q_cooldown_secs`, `m_affinity`,
  `hash_bias`, `l_band`, `cost_class`

The record SHOULD include up to three next-best feasible alternatives with
`credential_id`, `model_slug`, and `score`.

Replay tooling is not required in v1; the log contract MUST be stable for offline
incident analysis.

#### Scenario: Trace supports incident replay

- **WHEN** a routed request completes (success or terminal failure)
- **THEN** structured route trace contains winner hop 0 score breakdown
- **AND** `plan_snapshot_ts` is present
- **AND** no message body or prompt text is required to explain the hop-0 choice

#### Scenario: Replan emits second snapshot

- **WHEN** the router performs a plan rebuild after initial plan exhaustion
- **THEN** route trace records `plan_rebuilds=1`
- **AND** the terminal trace references the snapshot used for the successful or final hop

