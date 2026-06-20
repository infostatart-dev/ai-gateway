## ADDED Requirements

### Requirement: Post-429 upstream block syncs pacing oracle

The gateway MUST, when upstream returns HTTP 429 classified as `FailoverClass::QuotaExhausted` or RPM
exhaustion on a `per-model` provider:

1. Apply model-level cooldown until resolved reset instant (header or classifier)
2. Call `PacingGate::apply_upstream_block(until)` for the same `(credential, model)` pacing scope
3. Ensure the quota oracle reports `callable == false` until that instant for subsequent plans

#### Scenario: free-models-per-day blocks until reset header

- **WHEN** OpenRouter returns 429 with `free-models-per-day` and `X-RateLimit-Reset`
- **THEN** pacing scope for `(openrouter-default, nvidia/nemotron-3-nano-30b-a3b:free)` is blocked until reset
- **AND** the next plan excludes that pair without HTTP

#### Scenario: Gemini RPM 429 blocks pair until retry-after

- **WHEN** Gemini returns 429 RPM for `(gemini-free-2, gemini-3-flash-preview)` with `Retry-After: 60`
- **THEN** model cooldown and pacing block align to now + 60s
- **AND** oracle `callable` is false for 60s

---

### Requirement: Scope classification tests include post-429 block

The gateway SHALL ship unit tests verifying that `apply_upstream_block` after 429
classification makes `QuotaOracle.callable == false` until the block expires.

#### Scenario: Oracle block after quota exhausted

- **WHEN** a simulated 429 `QuotaExhausted` is classified for a per-model pair
- **THEN** immediate oracle peek returns `callable == false`
- **AND** after advancing test clock past reset, `callable == true`
