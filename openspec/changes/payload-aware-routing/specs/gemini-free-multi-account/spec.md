## MODIFIED Requirements

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
- **AND** additional free slots `gemini-free-2`..`gemini-free-4` are configured
- **THEN** the gateway does NOT dispatch the same request to the remaining free slots
- **AND** falls back to the paid `gemini-default` slot or the next provider

#### Scenario: Overload skips remaining free siblings
- **WHEN** slot `gemini-free` returns a `503` overload response
- **THEN** the gateway does NOT dispatch the same request to the remaining free slots
- **AND** falls back to the paid `gemini-default` slot or the next provider

### Requirement: Paid Gemini slot unchanged
The gateway SHALL keep credential slot `gemini-default` as the paid tier-3
Gemini account with a higher `budget-rank` than free slots. Adding free
multi-account slots SHALL NOT remove or repurpose `gemini-default`. After all
free Gemini slots are exhausted or skipped for a request, the gateway SHALL
attempt `gemini-default` once before abandoning the Gemini provider for that
request.

#### Scenario: Paid slot remains distinct
- **WHEN** both `gemini-free` and `gemini-default` secrets are configured
- **THEN** `gemini-default` retains tier `tier-3`
- **AND** free slots are preferred before `gemini-default` in budget-aware ordering

#### Scenario: Paid slot attempted after free slots are skipped
- **WHEN** all configured free Gemini slots are exhausted or skipped for a request
- **AND** `gemini-default` is configured and not in cooldown
- **THEN** the gateway attempts the request on `gemini-default` before moving off Gemini
