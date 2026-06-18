# quota-headroom-scheduling

## Purpose

Capture live free-tier quota headroom at route-plan time from embedded catalog
limits and in-process pacing gates, so the planner skips saturated models before
HTTP and parallel work units do not collide on the same exhausted credential.

## ADDED Requirements

### Requirement: Quota snapshot at plan time

Before building a route chain, the gateway SHALL construct a `QuotaSnapshot` from:

1. Per-model `PacingGate` state (RPM, TPM, RPD, concurrent) resolved via
   `ProviderLimitCatalog` and existing pacing registry scopes
2. Credential health circuit state
3. Active model and slot cooldown timers
4. Estimated request tokens from payload budget

The snapshot SHALL expose `headroom_score(credential_id, model_slug) -> f64` where
`0.0` means **no viable headroom** within `max_cooldown_wait`.

#### Scenario: RPD-exhausted model scores zero

- **WHEN** pacing daily window shows `rpd_remaining == 0` for
  `gemini-3-flash-preview` on `gemini-free-8`
- **THEN** `headroom_score(gemini-free-8, gemini-3-flash-preview)` is `0.0`

#### Scenario: Available RPM scores positive

- **WHEN** pacing gate reports `peek_next_wait == 0` for a model
- **AND** daily headroom remains
- **THEN** `headroom_score` is greater than `0.0`

### Requirement: Planner excludes zero-headroom candidates

Route chain planning SHALL omit any candidate whose `headroom_score` is `0.0` from
the initial plan and from replan passes.

Omission SHALL NOT increment provider-stats attempt counters.

#### Scenario: Saturated model skipped without HTTP

- **WHEN** `gemini-3-flash-preview` on `gemini-free-9` has zero headroom in the snapshot
- **AND** `gemini-3.1-flash-lite` on the same slot has positive headroom
- **THEN** the plan includes the flash-lite hop
- **AND** provider-stats shows no new attempt on `gemini-3-flash-preview` for that inbound request

### Requirement: Headroom-aware parallel work unit spread

The planner SHALL assign first-hop credentials for concurrent requests with distinct
work unit ids and the same invoker name using:

1. Filter to credentials with `headroom_score > 0` for the target model pool
2. Stable hash spread among survivors (see `route-chain-planning`)
3. When survivors count is less than concurrent work units, remaining work units
   SHALL start with the next best scored hop (alternate credential or provider),
   not a zero-headroom credential

#### Scenario: Two headroom keys three work units

- **WHEN** three concurrent requests arrive with work units `unit-1`, `unit-2`, `unit-3`
- **AND** only `gemini-free-9` and `gemini-free-10` have headroom for the preferred model
- **THEN** `unit-1` and `unit-2` first hops use distinct gemini credentials
- **AND** `unit-3` first hop is not `gemini-free-9` or `gemini-free-10` if both are
  headroom-zero at plan instant, OR uses openrouter/alternate provider with headroom

#### Scenario: Headroom updates between calls

- **WHEN** `unit-1` exhausts RPM on `gemini-free-9`
- **AND** `unit-2` plans milliseconds later
- **THEN** `unit-2` snapshot reflects reduced headroom on `gemini-free-9`
- **AND** spread may select `gemini-free-10` even if hash favored `gemini-free-9`

### Requirement: Peek next wait without acquiring permit

`PacingGate` SHALL expose `peek_next_wait(estimated_tokens) -> Duration` that
computes the same wait as `acquire` would, without taking a concurrent permit or
mutating counters.

Dynamic cooldown merge and quota snapshot SHALL use this API.

#### Scenario: Peek is read-only

- **WHEN** `peek_next_wait` is called twice without an intervening `acquire`
- **THEN** both calls return the same duration
- **AND** pacing counters are unchanged

### Requirement: Free-tier catalog limits drive pacing scopes

Quota snapshot resolution SHALL use embedded `provider-limits.yaml` per-model
entries (RPM, TPM, RPD) for `quota-profile: per-model` providers (Gemini free,
OpenRouter free) without duplicate limit configuration.

#### Scenario: Catalog RPD applies to snapshot

- **WHEN** the embedded catalog defines `gemini-3-flash-preview` with `rpd: 20` on gemini free tier
- **THEN** pacing scope for that model uses `rpd: 20` in snapshot headroom calculation
