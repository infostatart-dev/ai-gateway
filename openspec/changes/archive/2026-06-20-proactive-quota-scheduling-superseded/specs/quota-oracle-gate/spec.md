## ADDED Requirements

### Requirement: Quota oracle answers callability per credential and model

The gateway SHALL expose a `QuotaOracle` (or equivalent module) that, for each
`(credential_id, model_slug)` pair, returns whether an upstream HTTP attempt is
permitted at the current instant and when the pair becomes callable again.

The oracle SHALL combine, taking the maximum blocking instant among:

1. `PacingGate::peek_next_wait` (RPM, TPM, concurrent)
2. Daily RPD headroom (`daily_headroom_available == false` ⇒ blocked until next daily window)
3. Model-level cooldown (`ModelCooldownKey`)
4. Slot-level cooldown on the credential
5. Explicit upstream block applied after quota exhaustion (`apply_upstream_block`)

`callable` SHALL be `true` only when all sources report zero additional wait.

#### Scenario: RPM wait blocks call

- **WHEN** `peek_next_wait` returns 45s for `(gemini-free-2, gemini-3-flash-preview)`
- **THEN** `callable` is `false`
- **AND** `next_available_at` is approximately now + 45s
- **AND** `blocked_reason` is `rpm`

#### Scenario: Callable when all sources clear

- **WHEN** pacing peek returns 0
- **AND** daily headroom remains
- **AND** no model or slot cooldown is active
- **THEN** `callable` is `true`

---

### Requirement: Strict zero-wait headroom rule

`QuotaSnapshot.headroom_score` SHALL be `0.0` when `callable` is `false` for the pair.

`headroom_score` SHALL be greater than `0.0` only when `next_wait == 0` and daily headroom
remains.

The gateway SHALL NOT use `max_cooldown_wait` as a threshold for positive headroom on planned hops.

#### Scenario: Sub-second RPM wait scores zero

- **WHEN** `peek_next_wait` returns 500ms
- **THEN** `headroom_score` is `0.0`
- **AND** the pair is excluded from the route plan

#### Scenario: Zero wait scores positive

- **WHEN** `peek_next_wait` returns 0
- **AND** daily headroom remains
- **THEN** `headroom_score` is greater than `0.0`

---

### Requirement: Repeat upstream 429 on blocked pair is a gateway violation

The gateway MUST treat an upstream HTTP 429 on `(credential_id, model_slug)` as a scheduling
violation when the oracle reported `callable == false` for that pair immediately before the
attempt. In that case the gateway MUST:

1. Increment metric `gateway_repeat_429_violations_total`
2. Record `repeat_429_violation=true` on the route trace for that hop
3. NOT extend cooldown duration beyond the already-scheduled `next_available_at`

#### Scenario: Second 429 on quota-blocked nemotron

- **GIVEN** `(openrouter-default, nvidia/nemotron-3-nano-30b-a3b:free)` is oracle-blocked until T
  after `free-models-per-day`
- **WHEN** the walk still attempts HTTP and receives 429
- **THEN** `repeat_429_violation` is true on route trace
- **AND** `gateway_repeat_429_violations_total` increases by 1

#### Scenario: First 429 is not a violation

- **WHEN** the pair was oracle-callable before the attempt
- **AND** upstream returns 429 quota exhausted
- **THEN** `repeat_429_violation` is false
- **AND** post-429 block is applied per `per-model-exhaustion-scopes`

---

### Requirement: Hop-time oracle re-peek

Before each planned upstream attempt in the failover walk, the gateway SHALL re-invoke the oracle
for that candidate. If `callable` is `false`, the hop SHALL be skipped without incrementing
provider-stats attempt counters for that pair.

#### Scenario: Ladder second hop skipped after first hop consumes RPM

- **GIVEN** hop 1 on `(gemini-free, gemini-3-flash-preview)` succeeds and consumes RPM budget
- **WHEN** hop 2 plans `gemini-3.1-flash-lite` on the same slot
- **AND** re-peek shows slot RPM wait > 0 for flash-lite
- **THEN** hop 2 is skipped without HTTP
- **AND** walk proceeds to next planned candidate
