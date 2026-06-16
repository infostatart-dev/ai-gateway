# gemini-free-multi-account

## Purpose

Spread autodefault Gemini traffic across up to four free-tier Google AI Studio
API keys with per-slot env resolution, round-robin, and isolated cooldown/failover
before falling back to paid `gemini-default` or other providers.

## Requirements

### Requirement: Four free-tier Gemini credential slots
The gateway SHALL define four distinct Gemini credential slots for free-tier AI Studio keys: `gemini-free`, `gemini-free-2`, `gemini-free-3`, and `gemini-free-4`. Each slot SHALL use `tier: free` and the same `budget-rank` as the existing `gemini-free` slot.

#### Scenario: Embedded credential catalog lists four free slots
- **WHEN** the embedded credential registry is loaded
- **THEN** slots `gemini-free`, `gemini-free-2`, `gemini-free-3`, and `gemini-free-4` are present
- **AND** each slot maps to provider `gemini` with tier `free`

### Requirement: Per-slot environment resolution
The gateway SHALL resolve each free Gemini slot only from its universal credential env var (`AI_GATEWAY_CREDENTIAL_<ID>` with hyphens mapped to underscores). Legacy env names `GEMINI_FREE_TIER_API_KEY` and `GEMINI_FREE_TIER_APIKEY` SHALL apply only to `gemini-free`.

#### Scenario: Second free key resolves from its own env var
- **WHEN** `AI_GATEWAY_CREDENTIAL_GEMINI_FREE_2` is set and non-empty
- **THEN** credential slot `gemini-free-2` is registered at startup
- **AND** the secret is not shared with `gemini-free`

#### Scenario: Missing slot secret is skipped
- **WHEN** `AI_GATEWAY_CREDENTIAL_GEMINI_FREE_3` is unset or empty
- **THEN** slot `gemini-free-3` is omitted from the credential registry
- **AND** startup completes without error

#### Scenario: Legacy free-tier env maps to first slot only
- **WHEN** only `GEMINI_FREE_TIER_API_KEY` is set (no universal vars)
- **THEN** only `gemini-free` resolves
- **AND** slots `gemini-free-2` through `gemini-free-4` remain absent

### Requirement: Isolated cooldown and failover per free slot
The gateway SHALL track cooldown and failure state per credential slot ID. When a free Gemini slot returns rate-limit or quota errors, failover SHALL try other configured Gemini free slots before cooling down the entire Gemini provider or moving to a different provider.

#### Scenario: Rate limit on one free slot fails over to a sibling
- **WHEN** slot `gemini-free` returns HTTP 429 for a model request
- **AND** slot `gemini-free-2` is configured and not in cooldown
- **THEN** the gateway retries the request using `gemini-free-2`
- **AND** cooldown for `gemini-free` does not block `gemini-free-2`

### Requirement: Round-robin among free Gemini slots
The gateway SHALL distribute eligible chat requests across configured free Gemini slots using round-robin when multiple free slots support the same `(provider, model)` candidate set.

#### Scenario: Repeated requests rotate free accounts
- **WHEN** four free Gemini slots are configured for the same model
- **AND** four consecutive chat requests are routed to Gemini at the free tier
- **THEN** the first selected credential slot rotates across the four accounts
- **AND** no single slot receives all four first attempts

### Requirement: Paid Gemini slot unchanged
The gateway SHALL keep credential slot `gemini-default` as the paid tier-3 Gemini account with a higher `budget-rank` than free slots. Adding free multi-account slots SHALL NOT remove or repurpose `gemini-default`.

#### Scenario: Paid slot remains distinct
- **WHEN** both `gemini-free` and `gemini-default` secrets are configured
- **THEN** `gemini-default` retains tier `tier-3`
- **AND** free slots are preferred before `gemini-default` in budget-aware ordering

### Requirement: Documentation, tests, and release version
The gateway SHALL document the four-key free Gemini setup (env vars and relationship to `gemini-default`), SHALL test credential registration, env resolution, round-robin, and sibling failover without live API keys in CI, and SHALL ship this capability in release **`0.3.0-beta.12`**.

#### Scenario: Contributor verifies multi-account setup
- **WHEN** tests run for Gemini free multi-account support
- **THEN** four-slot registry parsing, per-slot env resolution, round-robin ordering, and sibling failover are covered
- **AND** docs describe all four `AI_GATEWAY_CREDENTIAL_GEMINI_FREE*` variables
