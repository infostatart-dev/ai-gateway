# provider-observability

## ADDED Requirements

### Requirement: Emulated stack exposes production observability contract

When the gateway runs in the emulated autodefault dev stack, it SHALL expose the same
provider observability surfaces as production:

- OpenTelemetry `gateway_provider_*` instruments
- `GET /v1/observability/provider-stats` (in-memory totals since process start)
- `X-Gateway-Provider-Usage` on terminal responses when enabled

Recorded token totals SHALL reflect gateway token estimation from request/response bodies — not
hardcoded emulator constants.

#### Scenario: provider-stats accumulates during emulated load

- **WHEN** an operator sends multiple autodefault requests with the canonical model against
  the emulated stack
- **THEN** `GET /v1/observability/provider-stats` returns monotonically increasing
  `calls.attempts` per `(provider, credential)` row
- **AND** rows appear only for providers that actually received upstream traffic

#### Scenario: Fat payload token totals in stats

- **WHEN** an autodefault request uses a routing_load-scale fat body
- **AND** the upstream returns token-faithful `usage`
- **THEN** provider-stats token fields (or linked usage JSON) reflect large prompt token counts
- **AND** values are not stuck at a constant stub like 6 prompt tokens

#### Scenario: Terminal usage header

- **WHEN** a terminal autodefault response succeeds through the emulated stack
- **THEN** the response includes `X-Gateway-Provider-Usage` JSON with `provider`, `credential`,
  and token fields consistent with upstream `usage` and stats recording

#### Scenario: Emulated 429 recorded

- **WHEN** the emulator returns HTTP 429 for an upstream attempt
- **THEN** `gateway_provider_calls_total{outcome=rate_limited}` increments
- **AND** failover attempts show incremented `attempt_index` when routing succeeds on a later hop

#### Scenario: Latency histograms reflect token-proportional delay

- **WHEN** the emulator applies base + per-token latency
- **THEN** `gateway_provider_request_duration_ms` samples for fat payloads exceed samples for
  hello payloads on the same provider
