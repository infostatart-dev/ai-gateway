## MODIFIED Requirements

### Requirement: Quota snapshot at plan time

Before building a route chain, the gateway SHALL construct a `QuotaSnapshot` from:

1. Per-model `PacingGate` state (RPM, TPM, RPD, concurrent) resolved via
   `ProviderLimitCatalog` and existing pacing registry scopes
2. Credential health circuit state
3. Active model and slot cooldown timers
4. Estimated request tokens from payload budget
5. Upstream quota blocks applied after prior 429 responses (`apply_upstream_block`)

The snapshot SHALL expose `headroom_score(credential_id, model_slug) -> f64` where
`0.0` means **not callable** (any positive `next_wait`, exhausted daily headroom, cooldown, or
upstream block).

#### Scenario: RPD-exhausted model scores zero

- **WHEN** pacing daily window shows `rpd_remaining == 0` for
  `gemini-3-flash-preview` on `gemini-free-8`
- **THEN** `headroom_score(gemini-free-8, gemini-3-flash-preview)` is `0.0`

#### Scenario: Available RPM scores positive

- **WHEN** pacing gate reports `peek_next_wait == 0` for a model
- **AND** daily headroom remains
- **AND** no upstream block is active
- **THEN** `headroom_score` is greater than `0.0`

#### Scenario: Sub-second RPM wait scores zero

- **WHEN** `peek_next_wait` returns any duration greater than zero
- **THEN** `headroom_score` is `0.0`

---

### Requirement: Planner excludes zero-headroom candidates

Route chain planning SHALL omit any candidate whose `headroom_score` is `0.0` from
the initial plan and from replan passes.

Omission SHALL NOT increment provider-stats attempt counters.

The planner SHALL NOT sleep or wait for cooldown on excluded candidates during plan construction.

#### Scenario: Saturated model skipped without HTTP

- **WHEN** `gemini-3-flash-preview` on `gemini-free-9` has zero headroom in the snapshot
- **AND** `gemini-3.1-flash-lite` on the same slot has positive headroom
- **THEN** the plan includes the flash-lite hop
- **AND** provider-stats shows no new attempt on `gemini-3-flash-preview` for that inbound request

#### Scenario: RPM-waiting model skipped without sleep

- **WHEN** `peek_next_wait` returns 2s for a candidate at plan time
- **THEN** the candidate is omitted from the plan
- **AND** the planner does not sleep 2s before continuing

---

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
