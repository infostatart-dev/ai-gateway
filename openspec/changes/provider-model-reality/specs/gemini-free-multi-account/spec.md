## ADDED Requirements

### Requirement: Intra-slot ladder-only failover on per-model providers

The gateway SHALL restrict intra-slot failover on a single credential to models
listed in `provider-ladders.yaml` when the provider has `quota-profile:
per-model` and a configured ladder. The gateway SHALL NOT attempt other
`providers.yaml` models on the same credential during the same request walk.
Inter-slot failover to other `gemini-free*` siblings SHALL occur only after
every ladder model on the current credential is exhausted, gated, or in model
cooldown.

#### Scenario: 404 on ladder slug tries next ladder model same slot

- **WHEN** `gemini-free-8` returns 404 for an invalid fast-band slug
- **AND** `gemini-3.1-flash-lite` is the next capacity-band model on the ladder
- **THEN** the gateway attempts `gemini-3.1-flash-lite` on `gemini-free-8`
- **AND** does not insert `gemini-free-8` into `failed_credentials`

#### Scenario: Dead providers.yaml slug not attempted on free slot

- **WHEN** `gemini-1.5-flash` is absent from the free ladder
- **AND** intent mode selects Gemini free tier
- **THEN** `gemini-1.5-flash` is not attempted on any `gemini-free*` credential

#### Scenario: Inter-slot after full ladder

- **WHEN** every ladder model on `gemini-free-8` is unavailable for the request
- **THEN** the gateway proceeds to `gemini-free-9` (or next sibling)
- **AND** does not attempt `gemini-default` before exhausting free siblings' ladders

---

### Requirement: Per-model 404 does not retire free slot

The gateway SHALL retire only `(credential_id, model)` for the request walk when
Gemini (`quota-profile: per-model`) receives HTTP 404 or unsupported-model HTTP
400 on one upstream slug. The gateway SHALL NOT add the credential to
`failed_credentials` solely because one ladder slug returned 404.

#### Scenario: Phantom slug does not block sibling models on slot

- **WHEN** `gemini-3.5-flash-preview` returns 404 on `gemini-free-8`
- **THEN** `gemini-3.1-flash-lite` on `gemini-free-8` remains eligible in the same request

---

### Requirement: Free stability band uses quota-backed models only

The Gemini free-tier stability band SHALL contain only models with non-zero free
quota in the embedded limit catalog. The gateway SHALL NOT place `gemini-2.5-pro`
in the free-tier ladder stability band.

#### Scenario: Stability escalates to flash-lite not pro

- **WHEN** fast and capacity bands on `gemini-free-8` are exhausted
- **AND** stability band is reached
- **THEN** the gateway attempts `gemini-2.5-flash-lite` (or configured free stability slug)
- **AND** does not attempt `gemini-2.5-pro` on the free tier

---

## MODIFIED Requirements

### Requirement: Isolated cooldown and failover per free slot

The gateway SHALL track cooldown and failure state per credential slot ID. When
a free Gemini slot returns a **transient per-minute** rate-limit error (RPM
`429` without daily-quota exhaustion) on a **per-model** provider, failover SHALL
retire only the `(credential, model)` pair and continue the **intra-slot ladder**
before trying other credential slots.

When a free Gemini slot returns **per-model daily quota exhaustion** for one slug,
failover SHALL retire that model only and continue the ladder on the same slot.

When a free Gemini slot returns **HTTP 503 high demand** on a per-model provider,
the gateway SHALL apply a **short slot cooldown** on that credential and SHALL
continue the intra-slot ladder for the current request when additional ladder
models remain. The gateway SHALL NOT skip all free Gemini siblings solely because
one model on one slot returned 503 high demand.

When a free Gemini slot returns **project billing cap** exhaustion, the gateway
SHALL skip remaining free Gemini siblings for that request and fall back to
`gemini-default` or the next provider.

#### Scenario: Rate limit on one model continues ladder same slot

- **WHEN** slot `gemini-free-8` returns a transient RPM HTTP 429 for `gemini-3-flash-preview`
- **AND** `gemini-3.5-flash` has remaining quota on the same slot
- **THEN** the gateway retries on `gemini-3.5-flash` on `gemini-free-8`
- **AND** does not skip to `gemini-free-9` yet

#### Scenario: Per-model daily quota does not skip free siblings

- **WHEN** slot `gemini-free-8` returns per-model daily quota exhaustion for `gemini-3-flash-preview` only
- **AND** `gemini-3.1-flash-lite` has remaining quota on the same slot
- **THEN** the gateway retries on `gemini-3.1-flash-lite` on `gemini-free-8`
- **AND** does not skip all free Gemini siblings

#### Scenario: 503 high demand continues ladder when models remain

- **WHEN** slot `gemini-free-8` returns HTTP 503 with high-demand body on `gemini-3.5-flash`
- **AND** `gemini-3.1-flash-lite` is next on the ladder
- **THEN** the gateway attempts `gemini-3.1-flash-lite` on `gemini-free-8`
- **AND** applies short cooldown on `gemini-free-8` for subsequent requests

#### Scenario: Project billing cap skips free siblings

- **WHEN** slot `gemini-free` returns project billing cap exhaustion
- **THEN** the gateway skips remaining free Gemini siblings for that request
- **AND** falls back to `gemini-default` or the next provider

#### Scenario: Rate limit on one free slot fails over to a sibling after ladder

- **WHEN** every ladder model on `gemini-free` is exhausted or in cooldown
- **AND** slot `gemini-free-2` is configured and not in cooldown
- **THEN** the gateway retries using `gemini-free-2` starting at the fast ladder band
- **AND** cooldown for `gemini-free` does not block `gemini-free-2`

---

### Requirement: Paid Gemini slot unchanged

The gateway SHALL keep credential slot `gemini-default` as the paid tier-3
Gemini account with a higher `budget-rank` than free slots. Adding free
multi-account slots SHALL NOT remove or repurpose `gemini-default`. After all
free Gemini slots exhaust their **ladder walks** for a request, the gateway SHALL
attempt `gemini-default` once before abandoning the Gemini provider for that
request.

#### Scenario: Paid slot remains distinct

- **WHEN** both `gemini-free` and `gemini-default` secrets are configured
- **THEN** `gemini-default` retains tier `tier-3`
- **AND** free slots are preferred before `gemini-default` in budget-aware ordering

#### Scenario: Paid slot attempted after free ladder exhausted

- **WHEN** all configured free Gemini slots have exhausted their ladder models for a request
- **AND** `gemini-default` is configured and not in cooldown
- **THEN** the gateway attempts the request on `gemini-default` before moving off Gemini

#### Scenario: Paid slot not used while free ladder has headroom

- **WHEN** `gemini-free-8` still has `gemini-3.1-flash-lite` available on the ladder
- **THEN** the gateway does not attempt `gemini-default` for that request
