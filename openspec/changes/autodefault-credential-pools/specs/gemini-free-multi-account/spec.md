## MODIFIED Requirements

### Requirement: Four free-tier Gemini credential slots

The gateway SHALL define **sixteen** distinct Gemini credential slots for
free-tier AI Studio keys: `gemini-free`, `gemini-free-2`, …, `gemini-free-16`.
Each slot SHALL use `tier: free`, `cost-class: free`, and the same
`budget-rank` as the existing `gemini-free` slot.

#### Scenario: Embedded credential catalog lists sixteen free slots

- **WHEN** the embedded credential registry is loaded
- **THEN** slots `gemini-free` through `gemini-free-16` are present
- **AND** each slot maps to provider `gemini` with tier `free`

### Requirement: Per-slot environment resolution

The gateway SHALL resolve each free Gemini slot only from its secrets-file entry
under `credentials.<id>.api-key` (or legacy env `AI_GATEWAY_CREDENTIAL_<ID>` when
secrets file is not used). Legacy env names `GEMINI_FREE_TIER_API_KEY` and
`GEMINI_FREE_TIER_APIKEY` SHALL apply only to `gemini-free`.

#### Scenario: Ninth free key resolves from its own slot

- **WHEN** `credentials.gemini-free-9.api-key` is set and non-empty
- **THEN** credential slot `gemini-free-9` is registered at startup
- **AND** the secret is not shared with `gemini-free`

#### Scenario: Missing slot secret is skipped

- **WHEN** `credentials.gemini-free-12.api-key` is unset or empty
- **THEN** slot `gemini-free-12` is omitted from the credential registry
- **AND** startup completes without error

#### Scenario: Legacy free-tier env maps to first slot only

- **WHEN** only `GEMINI_FREE_TIER_API_KEY` is set (no universal vars)
- **THEN** only `gemini-free` resolves
- **AND** slots `gemini-free-2` through `gemini-free-16` remain absent

### Requirement: Isolated cooldown and failover per free slot

The gateway SHALL track cooldown and failure state per credential slot ID. When
a free Gemini slot returns a **transient per-minute** rate-limit error (RPM
`429` without daily-quota exhaustion), failover SHALL try other configured
Gemini free slots before cooling down the entire Gemini provider or moving to a
different provider. When a free Gemini slot returns a **daily quota exhaustion**
error (`429 RESOURCE_EXHAUSTED` with daily-quota / `RetryInfo` indicating a long
reset) or a **`503` overload** response, the gateway SHALL skip the remaining
free Gemini siblings for that request and fall back to the paid `gemini-default`
slot or the next provider, rather than dispatching the same request to every
free slot.

#### Scenario: Rate limit on one free slot fails over to a sibling

- **WHEN** slot `gemini-free` returns a transient RPM HTTP 429 for a model request
- **AND** slot `gemini-free-2` is configured and not in cooldown
- **THEN** the gateway retries the request using `gemini-free-2`
- **AND** cooldown for `gemini-free` does not block `gemini-free-2`

#### Scenario: Daily quota exhaustion skips remaining free siblings

- **WHEN** slot `gemini-free` returns `429 RESOURCE_EXHAUSTED` indicating daily-quota exhaustion
- **AND** additional free slots `gemini-free-2`..`gemini-free-16` are configured
- **THEN** the gateway does NOT dispatch the same request to the remaining free slots
- **AND** falls back to the paid `gemini-default` slot or the next provider

#### Scenario: Overload skips remaining free siblings

- **WHEN** slot `gemini-free` returns a `503` overload response
- **THEN** the gateway does NOT dispatch the same request to the remaining free slots
- **AND** falls back to the paid `gemini-default` slot or the next provider

### Requirement: Round-robin among free Gemini slots

The gateway SHALL distribute eligible chat requests across configured free Gemini
slots using round-robin when multiple free slots support the same
`(provider, model)` candidate set.

#### Scenario: Repeated requests rotate sixteen free accounts

- **WHEN** sixteen free Gemini slots are configured for the same model
- **AND** sixteen consecutive chat requests are routed to Gemini at the free tier
- **THEN** the first selected credential slot rotates across all sixteen accounts
- **AND** no single slot receives all sixteen first attempts

### Requirement: Documentation, tests, and release version

The gateway SHALL document the sixteen-key free Gemini setup (secrets slot ids
and relationship to `gemini-default`), SHALL test credential registration,
per-slot resolution, sixteen-slot round-robin, and sibling failover without live
API keys in CI.

#### Scenario: Contributor verifies sixteen-slot setup

- **WHEN** tests run for Gemini free multi-account support
- **THEN** sixteen-slot registry parsing, per-slot resolution, round-robin ordering, and sibling failover are covered
- **AND** docs describe `gemini-free` through `gemini-free-16` in `docs/credentials.md`
