# routing-load-verification

## ADDED Requirements

### Requirement: Level 3 external verification via emulated universal upstream

Level 3 routing load verification SHALL use the emulated autodefault dev stack: real HTTP to
gateway `:8080`, universal upstream emulator, synthetic secrets — **no live API keys**.

#### Scenario: External HTTP entrypoint

- **WHEN** an operator starts the emulated stack
- **THEN** load generators MAY target `POST /router/autodefault/chat/completions`
- **AND** use model `openai/gpt-5.4-nano` (same as CLI banner)

### Requirement: Assertions via provider-stats and token-faithful usage

L3 tests SHALL assert routing outcomes via:

- `GET /v1/observability/provider-stats` per-row `(provider, credential)` attempt totals
- Response `usage` / `X-Gateway-Provider-Usage` scaling with payload size on fat bodies

L3 tests SHALL NOT treat constant stub token counts (e.g. 6+1) as success for fat payloads.

#### Scenario: Fat payload acceptance

- **WHEN** external load sends a routing_load-scale fat JSON schema body through autodefault
- **THEN** HTTP 200 responses include `usage.prompt_tokens` greater than 1000
- **AND** provider-stats reflects the routing hop with non-stub token totals

#### Scenario: Fairness across credentials

- **WHEN** load is configured to exercise multiple credentials for the same provider
- **THEN** automation reads provider-stats rows per credential
- **AND** MAY assert fairness bands on `calls.attempts` distribution

#### Scenario: Failover visible in stats

- **WHEN** load forces failover (e.g. first provider returns 429 via emulator admin profile or
  RPM exhaustion)
- **THEN** provider-stats shows attempts on multiple provider/credential rows
- **AND** `routing.requests_with_failover` increases when applicable

#### Scenario: k6 summary polls stats

- **WHEN** `benchmarks/suite/routing-autodefault.js` completes
- **THEN** `handleSummary` fetches provider-stats for operator inspection

### Requirement: L1 and L2 verification paths remain unchanged

In-process `routing_load` and Harness L2 tests SHALL remain the fast CI path. L3 emulated
stack verification is additive and MUST NOT replace L1/L2.

#### Scenario: CI layering

- **WHEN** CI runs routing verification
- **THEN** L1/L2 tests do not require the emulator process
- **AND** L3 may run in a separate job or nightly with `mise dev:emulated` prerequisites
