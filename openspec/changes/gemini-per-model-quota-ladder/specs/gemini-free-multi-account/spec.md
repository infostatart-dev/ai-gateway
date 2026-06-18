## MODIFIED Requirements

### Requirement: Isolated cooldown and failover per free slot

The gateway SHALL track cooldown and failure state per credential slot ID **and**
per upstream model within the slot when the provider exposes per-model quotas.
When a free Gemini slot returns a **transient per-minute** rate-limit error (RPM
`429` without daily-quota exhaustion) for **one model**, failover SHALL first
try other models on the **same** credential via the model ladder before trying
other configured Gemini free **credential** slots. When a free Gemini slot returns
**per-model daily quota exhaustion** for one slug, failover SHALL walk remaining
ladder models on the same credential before marking the slot exhausted.

When a free Gemini slot returns **project-wide** daily quota exhaustion, billing
cap, or **`503` overload** that applies to the whole AI Studio project, the
gateway SHALL skip the remaining free Gemini siblings for that request and fall
back to the paid `gemini-default` slot or the next provider.

#### Scenario: Rate limit on one model tries ladder then sibling credential

- **WHEN** slot `gemini-free-8` returns a transient RPM HTTP 429 for `gemini-3-flash-preview`
- **AND** `gemini-3.1-flash-lite` on the same slot has remaining quota
- **THEN** the gateway retries on `gemini-3.1-flash-lite` on `gemini-free-8`
- **AND** does not immediately fail over to `gemini-free-9`

#### Scenario: Per-model daily cap does not skip all siblings

- **WHEN** slot `gemini-free-8` returns per-model daily quota exhaustion for `gemini-3-flash-preview` only
- **AND** other ladder models on `gemini-free-8` remain eligible
- **THEN** the gateway does NOT skip `gemini-free-9` through `gemini-free-16` for that reason alone
- **AND** continues the intra-slot ladder

#### Scenario: Project-wide daily quota exhaustion skips remaining free siblings

- **WHEN** slot `gemini-free` returns `429 RESOURCE_EXHAUSTED` indicating **project-wide** daily-quota or billing cap
- **AND** additional free slots `gemini-free-2`..`gemini-free-16` are configured
- **THEN** the gateway does NOT dispatch the same request to the remaining free slots
- **AND** falls back to the paid `gemini-default` slot or the next provider

#### Scenario: Overload skips remaining free siblings

- **WHEN** slot `gemini-free` returns a `503` overload response affecting the whole project
- **THEN** the gateway does NOT dispatch the same request to the remaining free slots
- **AND** falls back to the paid `gemini-default` slot or the next provider

### Requirement: Round-robin among free Gemini slots

The gateway SHALL distribute eligible chat requests across configured free Gemini
**credential** slots using round-robin when multiple free slots support the same
`(provider, model)` candidate set. Round-robin applies at the **credential**
level after the per-credential model ladder is exhausted for the request walk.

#### Scenario: Repeated requests rotate sixteen free accounts

- **WHEN** sixteen free Gemini slots are configured for the same model
- **AND** sixteen consecutive chat requests are routed to Gemini at the free tier
- **THEN** the first selected credential slot rotates across all sixteen accounts
- **AND** no single slot receives all sixteen first attempts when all slots are healthy
