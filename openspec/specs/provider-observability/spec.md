# provider-observability

## Purpose

Expose comparable upstream provider telemetry through OpenTelemetry, an in-process REST
snapshot since last restart, and a single JSON response header — with consistent token,
latency, HTTP stability, and failover semantics.

## Requirements

### Requirement: Dual-layer observability scopes

The gateway SHALL maintain two distinct observability scopes: **upstream attempt** (each
provider/credential dispatch, including failed failovers) and **client request** (one inbound
call until the gateway responds to the caller).

#### Scenario: Failed hop attributed to failed provider only

- **WHEN** a router request tries `gemini-free`, receives `429`, then succeeds on `groq`
- **THEN** attempt-level metrics record two hops with `attempt_index=0` and `attempt_index=1`
- **AND** the `429` increments `gateway_provider_responses_by_status_total` for
  `provider=gemini-free`
- **AND** terminal tokens and the response header reflect `groq` only

#### Scenario: Client request counters stay terminal

- **WHEN** the same failover request succeeds on the second hop
- **THEN** `router_responses_total` increments once for the client request
- **AND** `routing.failover_rate` in REST reflects one client request with failover

### Requirement: Upstream attempt recording

The gateway SHALL record one observability event for every upstream dispatch attempt, labeled
with `attempt_index` starting at zero for the first hop in a client request.

#### Scenario: Direct proxy records a single attempt

- **WHEN** a client calls a provider through direct proxy without router failover
- **THEN** exactly one upstream attempt record is stored with `attempt_index=0`

### Requirement: Token usage counters per provider

For each upstream attempt with known or estimated usage, the gateway SHALL increment token
counters by `token_type` ∈ {`input`, `output`, `cached`, `reasoning`, `total`} with labels
`provider`, `credential`, `model`, and `usage_source`.

#### Scenario: Reported OpenAI-compatible usage

- **WHEN** an upstream returns `usage.prompt_tokens=100` and `usage.completion_tokens=40`
- **THEN** `gateway_provider_tokens_total{token_type=input,usage_source=reported}` increases
  by 100
- **AND** `gateway_provider_tokens_total{token_type=output,usage_source=reported}` increases
  by 40

#### Scenario: Missing usage on success with estimation enabled

- **WHEN** an upstream returns HTTP 200 without parseable usage and estimation is enabled
- **THEN** token counters increment with `usage_source=estimated`
- **AND** the attempt is classified as `outcome=success_degraded`

#### Scenario: Failed hop without body usage

- **WHEN** an upstream returns HTTP 429 before any usage block is available
- **THEN** no token counters increment for that hop
- **AND** status and outcome counters still increment

### Requirement: Generation latency per output token

The gateway SHALL record upstream generation efficiency as milliseconds per output token using
`(request_duration_ms - tfft_ms) / max(output_tokens, 1)` when `output_tokens >= 1`.

#### Scenario: Streaming completion with usage

- **WHEN** a streaming upstream attempt completes with `tfft_ms=300`, total duration
  `1200 ms`, and `output_tokens=50`
- **THEN** histogram `gateway_provider_generation_ms_per_output_token` records `18.0`
- **AND** histogram `gateway_provider_tfft_ms` records `300`

#### Scenario: Zero output tokens

- **WHEN** an attempt completes with `output_tokens=0` or unknown output tokens
- **THEN** no `gateway_provider_generation_ms_per_output_token` sample is emitted

### Requirement: Provider call quality outcomes

The gateway SHALL classify every upstream attempt into exactly one primary `outcome` value:
`success`, `success_degraded`, `client_error`, `server_error`, `rate_limited`, or `overload`.

#### Scenario: Successful call with reported usage

- **WHEN** upstream returns HTTP 200 and usage is provider-reported
- **THEN** `gateway_provider_calls_total{outcome=success}` increments

#### Scenario: Rate limit response

- **WHEN** upstream returns HTTP 429
- **THEN** `gateway_provider_calls_total{outcome=rate_limited}` increments

### Requirement: HTTP status stability counters

The gateway SHALL increment `gateway_provider_responses_by_status_total` with the exact
upstream HTTP `status_code` label for every attempt.

#### Scenario: Status breakdown available in OTEL

- **WHEN** provider `groq` returns five `200`, two `429`, and one `500` in a window
- **THEN** exported counters include separate totals for `status_code=200`, `429`, and `500`

### Requirement: OpenTelemetry export

All instruments defined in this capability SHALL export through the gateway's existing OTLP
metrics pipeline without requiring a separate exporter configuration.

#### Scenario: Collector receives attempt-level series

- **WHEN** OTLP export is enabled
- **THEN** `gateway_provider_*` instruments include `attempt_index` and `provider` labels

### Requirement: Public runtime REST snapshot since restart

The gateway SHALL expose unauthenticated `GET /v1/observability/provider-stats` returning
cumulative statistics since the current process started, with attempt-level provider rows and
a client-request `routing` summary block.

#### Scenario: Snapshot after mixed traffic

- **WHEN** an operator queries `/v1/observability/provider-stats` after traffic with failovers
- **THEN** the response includes per-provider attempt totals, `status_codes`, token sums,
  latency aggregates, `started_at`, `uptime_seconds`, and `routing.failover_rate`

#### Scenario: Same accessibility as health check

- **WHEN** a backend calls `/health` without credentials on a private deployment
- **THEN** `/v1/observability/provider-stats` is reachable under the same network trust
  model without API key authentication

#### Scenario: Process restart resets snapshot

- **WHEN** the gateway process restarts
- **THEN** subsequent responses show zeroed counters and a new `started_at`

### Requirement: Single JSON response header

On terminal client-facing completions, the gateway SHALL set exactly one attribution header
`X-Gateway-Provider-Usage` whose value is a compact JSON object describing terminal provider,
usage, latency, and routing summary.

#### Scenario: Successful router completion header

- **WHEN** a router request completes through provider `groq` with reported `input=100`,
  `output=40`, `tfft_ms=250`, generation duration `950 ms`, and two upstream hops
- **THEN** the client response includes one header `X-Gateway-Provider-Usage`
- **AND** its JSON value includes `"provider":"groq"`, `"usage":{"input":100,"output":40,
  "total":140,"source":"reported"}`, `"latency_ms":{"total":1200,"ttft":250,
  "generation_per_output_token":23.8}`, and `"routing":{"attempts":2,"failover":true}`

#### Scenario: Estimated usage in header

- **WHEN** terminal upstream returns HTTP 200 without provider usage and estimation yields
  `input=80`, `output=20`
- **THEN** the header JSON includes `"usage":{"source":"estimated","input":80,"output":20,...}`

#### Scenario: Headers disabled in config

- **WHEN** `observability.response_headers.enabled=false`
- **THEN** the gateway omits `X-Gateway-Provider-Usage`

### Requirement: Shared recording path

OTEL metrics, the runtime REST registry, and the JSON response header SHALL be populated from
the same upstream attempt recording function to prevent drift between export paths.

#### Scenario: OTEL and REST agree on token totals

- **WHEN** an attempt records `input=10` and `output=5` with `usage_source=reported`
- **THEN** OTEL token counters and the in-memory registry increment by the same amounts for
  that `(provider, credential)` key

### Requirement: Estimated usage labeling

The gateway SHALL default `observability.estimate_tokens` to enabled and MUST label estimated
token values with `usage_source=estimated` in OTEL, REST, response header JSON, and traces.

#### Scenario: Estimated tokens visible to operators

- **WHEN** estimation produces token values on a 200 response without provider usage
- **THEN** `"usage":{"source":"estimated",...}` appears in `X-Gateway-Provider-Usage`
- **AND** `outcome=success_degraded` is recorded for that attempt
