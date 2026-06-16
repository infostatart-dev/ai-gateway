# routing-load-verification

## Purpose

Concurrent and burst routing correctness verification for budget-aware autodefault
without live provider keys or token spend. Validates round-robin fairness, payload-aware
filtering under load, Gemini failover chains, ChatGPT Web last-resort invariants, and
pacing/shaper backpressure via provider observability stats.

## ADDED Requirements

### Requirement: Unified routing load test framework

The gateway SHALL provide a shared routing load verification framework with reusable
fixtures, stats assertion helpers, and a scenario catalog — not ad-hoc one-off tests.
New autodefault routing scenarios SHALL be addable by introducing a single scenario file
that reuses the shared fixture and assertion primitives.

#### Scenario: Contributor adds a new routing scenario

- **WHEN** a contributor implements a new autodefault routing concern
- **THEN** they add one scenario file under the routing load test tree
- **AND** reuse `RoutingLoadProfile` and stats assertion helpers without duplicating credential setup

### Requirement: No live credentials or token spend in CI

Routing load verification tests SHALL use synthetic credential secrets (e.g.
`free-{n}-key`, `paid-key`) loaded from test fixtures only. Tests SHALL NOT call live
provider base URLs or require production `AI_GATEWAY_CREDENTIAL_*` environment variables.

#### Scenario: CI runs without production secrets

- **WHEN** routing load tests execute in CI with the `testing` feature
- **THEN** all upstream dispatch goes to mocks or injected responses
- **AND** no real provider API keys are read from the environment

### Requirement: Assertions via provider-stats per credential

Routing load tests SHALL assert routing correctness primarily through
`GET /v1/observability/provider-stats`, using per-row `(provider, credential)` attempt
totals. Tests SHALL NOT depend on LLM response content quality for pass/fail.

#### Scenario: Fairness check uses credential rows

- **WHEN** a concurrent round-robin scenario completes
- **THEN** the test reads provider-stats rows for each expected `gemini-free*` credential
- **AND** compares attempt counts against tolerance bands

#### Scenario: Last-resort check uses zero attempts

- **WHEN** free Gemini credentials succeed for all client requests in a scenario
- **THEN** the provider-stats row for `chatgpt-web-default` shows zero attempts

### Requirement: Per-credential concurrent mock responses

The gateway test infrastructure SHALL support mock upstream responses keyed by
`ProviderCredentialId` so concurrent routing load scenarios are deterministic. The
existing global FIFO `push_test_call_response` queue SHALL remain available for
backward-compatible sequential tests.

#### Scenario: Concurrent failover uses credential-keyed mocks

- **WHEN** two concurrent requests fail over from different first-picked Gemini slots
- **THEN** each request receives the mock response configured for its credential chain
- **AND** responses do not cross-contaminate via a single global queue

### Requirement: Prod-like autodefault fixture profile

The framework SHALL ship a default `AutodefaultProdLike` fixture registering four free
Gemini credential slots (`gemini-free` through `gemini-free-4`), optional paid
`gemini-default`, and ChatGPT Web as paid-browser last resort. The fixture SHALL use
budget-aware-capability-after routing with decision enabled and fat `json_schema` chat
payloads representative of large-context structured-output requests.

#### Scenario: Fixture builds four free Gemini candidates

- **WHEN** the default routing load profile is initialized in a test
- **THEN** four distinct free Gemini credential slots resolve with distinct synthetic keys
- **AND** the router ranks them before paid Gemini and ChatGPT Web

#### Scenario: Parameterized free slot count

- **WHEN** a scenario configures `N` free Gemini slots where `1 <= N <= 4`
- **THEN** fairness assertions use `N` as the divisor for expected per-slot attempt bands

### Requirement: Concurrent round-robin fairness

The gateway SHALL distribute first-attempt credential selection across configured free
Gemini slots within a documented tolerance band under concurrent successful load. Default
tolerance SHALL be plus or minus 25 percent of the uniform share (client_requests divided
by N per slot).

#### Scenario: Thirty-two concurrent requests across four free slots

- **WHEN** four free Gemini slots are configured and thirty-two concurrent chat requests
  with fat `json_schema` bodies all succeed on the first upstream hop
- **THEN** each `gemini-free*` credential row shows terminal success attempts between
  six and ten inclusive (±25% of eight)
- **AND** `chatgpt-web-default` shows zero attempts
- **AND** `routing.failover_rate` is below one percent

### Requirement: Payload-aware filtering under load

Routing load tests SHALL verify that providers whose effective window is exceeded by
estimated input tokens receive zero upstream attempts while eligible providers receive
traffic under concurrent dispatch, not only in sequential single-request tests.

#### Scenario: Fat json_schema skips TPM-limited provider

- **WHEN** a concurrent scenario sends requests whose estimated input exceeds a groq model
  TPM cap but fits a Gemini free slot window
- **THEN** provider-stats shows zero attempts for groq credentials
- **AND** positive attempts on eligible Gemini free slots

### Requirement: Transient RPM failover to Gemini sibling

The gateway SHALL fail over to a sibling free Gemini slot for the same client request
when a free Gemini slot returns a transient RPM HTTP 429 under concurrent load, without
attempting ChatGPT Web.

#### Scenario: RPM 429 on first slot routes to sibling

- **WHEN** mock configuration returns transient RPM 429 for `gemini-free` and success for
  `gemini-free-2`
- **THEN** the client request succeeds with terminal credential `gemini-free-2`
- **AND** provider-stats records attempts on both slots for that failover chain
- **AND** `chatgpt-web-default` shows zero attempts

### Requirement: Daily quota skips remaining free siblings

The gateway SHALL NOT dispatch the same request to remaining free Gemini siblings when a
free Gemini slot returns daily-quota exhaustion, and SHALL attempt paid gemini-default
before ChatGPT Web.

#### Scenario: Daily quota on first free slot reaches paid Gemini

- **WHEN** mock returns daily-quota 429 for `gemini-free` and success for `gemini-default`
- **AND** additional free slots `gemini-free-2` through `gemini-free-4` are configured
- **THEN** the request does not attempt `gemini-free-2` through `gemini-free-4` for that
  client request
- **AND** terminal success is on `gemini-default`
- **AND** `chatgpt-web-default` shows zero attempts

### Requirement: ChatGPT Web last-resort invariant

ChatGPT Web (`chatgpt-web-default`) SHALL be attempted only when higher-priority free and
paid API credentials are unavailable, in cooldown, or payload-filtered out for the
request. While any configured free API credential succeeds for the request class under
test, ChatGPT Web SHALL receive zero attempts.

#### Scenario: Free Gemini success excludes ChatGPT Web

- **WHEN** concurrent requests succeed via free Gemini slots
- **THEN** `chatgpt-web-default` attempt count remains zero for the scenario window

#### Scenario: Exhausted free chain allows ChatGPT Web

- **WHEN** the fixture places all free API credentials in cooldown or mock failure
- **AND** ChatGPT Web is configured and mock-success enabled via injection
- **THEN** terminal attempts include `chatgpt-web-default`
- **AND** free Gemini slots were attempted or skipped according to policy before ChatGPT Web

### Requirement: Pacing serializes browser-session providers under burst

The gateway SHALL enforce provider pacing limits for concurrent client burst directed at
ChatGPT Web so at most one in-flight upstream completion per credential scope is active
at a time for the configured tier.

#### Scenario: Ten concurrent requests respect single concurrent slot

- **WHEN** ten concurrent requests are routed to `chatgpt-web-default` with pacing
  `concurrent: 1` and simulated time advancement
- **THEN** provider-stats never shows more than one in-flight attempt completing overlap
  within the same credential scope
- **AND** no panic or poisoned mutex occurs in pacing or router state

### Requirement: Decision shaper backpressure scenario

Routing load verification SHALL include a separate scenario for decision traffic shaper
limits so shaper rejection is not misread as router imbalance. When free-tier shaper
slots are exhausted, excess requests SHALL queue or fail with shaper error without
duplicate upstream attempts.

#### Scenario: Shaper limits concurrent free-tier acquisitions

- **WHEN** decision shaper `free-tier` limit is sixteen and thirty-two concurrent inbound
  requests arrive with decision enabled
- **THEN** at most sixteen requests hold free-tier shaper permits concurrently
- **AND** rejected or queued requests do not produce spurious upstream attempts

### Requirement: CI tiering and serial isolation

Level 1 and Level 2 routing load tests SHALL run on every PR in CI within a bounded time
budget. Tests sharing global router state (`CredentialRoundRobin`, `ProviderState`,
pacing registry) SHALL use serial test execution and isolated `AppState` or Harness per
test case.

#### Scenario: PR CI runs routing load suite

- **WHEN** CI executes `cargo test` with the `testing` feature
- **THEN** routing load verification tests complete without live network
- **AND** tests marked serial do not run concurrently with each other

### Requirement: Documentation and release version

The gateway SHALL document how to add routing load scenarios (fixture, stats assertions,
mock setup, serial constraints). This capability SHALL ship in release **`0.3.0-beta.21`**.

#### Scenario: Contributor reads routing load guide

- **WHEN** a contributor opens the routing load verification documentation
- **THEN** the doc explains Level 1 vs Level 2, stats assertion patterns, and how to add
  a scenario file without splitting the framework
