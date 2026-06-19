# credential-health-routing

## Purpose

Track upstream credential health at runtime, open circuits on persistently failing
slots, merge dynamic cooldown with catalog pacing, and feed health into candidate
ranking so dead Gemini/OpenRouter keys stop receiving attempts.

## ADDED Requirements

### Requirement: Rolling credential health window

The gateway SHALL maintain per `(provider, credential_id)` health counters over a
rolling window (default 5 minutes): total attempts, successes, rate-limited
failures, and other errors.

Health updates SHALL occur on every recorded upstream attempt in
`ProviderRuntimeRegistry`.

#### Scenario: Success increments healthy counter

- **WHEN** `gemini-free-9` returns HTTP 200
- **THEN** the health window for `(gemini, gemini-free-9)` records one success

#### Scenario: 429 increments rate-limited counter

- **WHEN** `gemini-free-8` returns HTTP 429
- **THEN** the health window records one rate-limited failure for that credential

### Requirement: Circuit-open on persistently failing credentials

The gateway SHALL mark a credential **circuit-open** when the rolling window has
at least 5 attempts AND success rate is below 10%.

Circuit-open credentials SHALL be excluded from route planning until
`circuit_open_until` elapses (default 15 minutes) or the credential records a
successful upstream response.

Auth failures (HTTP 401) on a credential SHALL immediately open the circuit for
that credential (slot scope).

#### Scenario: Dead Gemini key circuit opens

- **WHEN** `gemini-free-8` records 20 attempts and 1 success in the window
- **THEN** the credential enters circuit-open state
- **AND** route planning excludes `gemini-free-8` until TTL or success

#### Scenario: Success closes circuit

- **WHEN** `gemini-free-8` is circuit-open
- **AND** the next attempt on that credential succeeds
- **THEN** the circuit closes immediately

#### Scenario: Auth error opens slot circuit

- **WHEN** `mistral-default` returns HTTP 401
- **THEN** `mistral-default` enters circuit-open without waiting for window minimum

### Requirement: Model cooldown participates in ranking

`rank_candidates` SHALL consider per-model cooldown (`model_states`) when computing
`effective_budget_rank`, using the maximum remaining cooldown between slot-level
and model-level state for each candidate.

#### Scenario: Model on cooldown deprioritized

- **WHEN** `(gemini-free-9, gemini-3-flash-preview)` has 45s model cooldown remaining
- **AND** `(gemini-free-10, gemini-3-flash-preview)` has no cooldown
- **THEN** the gemini-free-10 candidate ranks before gemini-free-9 for the same model

### Requirement: Dynamic cooldown merges pacing wait

On upstream failure, the gateway SHALL set cooldown duration to the maximum of:

1. Duration from existing `classify_and_cooldown` (Retry-After, body class, YAML fallback)
2. `PacingGate::peek_next_wait(estimated_tokens)` for the candidate's pacing scope

#### Scenario: Pacing-saturated model gets longer cooldown

- **WHEN** a Gemini model returns RPM 429
- **AND** the per-model pacing gate reports 47s until next slot
- **THEN** applied cooldown is at least 47s even if YAML rate-limit fallback is 60s with no Retry-After

#### Scenario: Retry-After dominates pacing

- **WHEN** upstream returns `Retry-After: 120`
- **AND** pacing reports 10s next wait
- **THEN** applied cooldown is at least 120s

### Requirement: Slot circuit when all ladder models exhausted

The gateway MUST apply slot-level circuit-open for a credential when the provider
uses `quota-profile: per-model` and every model in the credential's active ladder
band is simultaneously in model cooldown or quota exhaustion, until the earliest
model cooldown expires.

#### Scenario: All fast models on slot cooling opens slot circuit

- **WHEN** all fast-band ladder models on `gemini-free-4` are in model cooldown
- **THEN** `gemini-free-4` is treated as circuit-open for planning
- **AND** failover does not walk each model on that slot individually
