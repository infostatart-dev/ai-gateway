## ADDED Requirements

### Requirement: Per-model profile activates unified routing stack

The gateway SHALL enable a unified per-model routing stack when a provider block in
`provider-limits.yaml` declares `quota-profile: per-model`. For that provider the stack SHALL
include: (1) pacing scope `CredentialModel { credential, wire_slug }`; (2) ladder-only candidate
filter when `provider-ladders.yaml` defines a ladder; (3) ladder-band rank ordering; (4)
`failed_models` retirement on model-scoped exhaustion; (5) budget-probe paid-route pre-skip when
`runtime-sources.key-info` is configured. Providers without `quota-profile: per-model` SHALL retain
existing per-slot or per-session semantics unchanged.

#### Scenario: OpenRouter opts into per-model stack

- **WHEN** embedded `provider-limits.yaml` sets `openrouter.quota-profile: per-model`
- **THEN** pacing gates for `openrouter-default` are keyed per wire slug
- **AND** ladder filter applies to OpenRouter free-tier candidates
- **AND** no OpenRouter-specific `if provider == OpenRouter` branch is required in router code

#### Scenario: Per-slot provider unchanged

- **WHEN** a provider has no `quota-profile` or `quota-profile: per-slot`
- **THEN** pacing scope remains credential-level
- **AND** ladder filter does not remove candidates

---

### Requirement: Ladder-only intra-slot walk

On a single credential for a `per-model` provider with a configured ladder, the gateway SHALL
attempt only wire slugs listed in `provider-ladders.yaml` for that `(provider, tier)` during one
request walk. Inter-slot failover to sibling credentials SHALL occur only after every ladder slug
on the current credential is exhausted, gated, or in model cooldown.

#### Scenario: OpenRouter nemotron exhausted continues to gpt-oss same slot

- **WHEN** `openrouter-default` returns 429 `free-models-per-day` for `nvidia/nemotron-3-nano-30b-a3b:free`
- **AND** `openai/gpt-oss-120b:free` is on the free ladder and not in cooldown
- **THEN** the gateway attempts `openai/gpt-oss-120b:free` on `openrouter-default` in the same request
- **AND** does not insert `openrouter-default` into `failed_credentials`

#### Scenario: Dead catalog slug not attempted

- **WHEN** a slug is absent from the provider ladder for the credential tier
- **THEN** that slug is not attempted on that credential during the request walk

---

### Requirement: Stability band escalates up on same credential

For `per-model` providers with a ladder `stability` band, the gateway SHALL attempt stability-band
models on the **same credential** before cross-provider failover when fast and capacity bands are
exhausted. Stability selection SHALL prefer **larger or higher-capacity** free models on the slot.
The gateway SHALL NOT select a stability-band model that is smaller or less capable than models
already attempted in the fast band on that slot. The gateway SHALL NOT downgrade below the client
intent `floor_tier`.

#### Scenario: OpenRouter stability prefers gpt-oss over nemotron

- **WHEN** fast and capacity bands on `openrouter-default` are exhausted for a fast-thinking request
- **AND** stability band includes `openai/gpt-oss-120b:free`
- **AND** `nvidia/nemotron-3-nano-30b-a3b:free` is only in deprioritized band
- **THEN** the gateway attempts `openai/gpt-oss-120b:free` before nemotron on the same credential

#### Scenario: Stability does not downgrade below intent floor

- **WHEN** a fast-thinking request has `floor_tier: fast-thinking`
- **AND** only fast-tier upstream remains on other providers
- **THEN** the gateway does not select fast-only slugs below the floor on OpenRouter stability hop

---

### Requirement: Ladder rank ordering replaces alphabetical model tie-break

Within the same effective budget rank and cost-class band, candidate ordering SHALL use
`ladder_rank` (fast before capacity before stability before deprioritized) before json_schema rank
and latency tie-breaks. The gateway SHALL NOT use raw lexical sort on wire slug as a primary
ordering key for `per-model` providers.

#### Scenario: gpt-oss ranked before nemotron on OpenRouter free tier

- **WHEN** both `openai/gpt-oss-120b:free` and `nvidia/nemotron-3-nano-30b-a3b:free` are eligible
- **AND** both have the same credential and budget rank
- **THEN** `openai/gpt-oss-120b:free` is ordered before `nvidia/nemotron-3-nano-30b-a3b:free`

---

### Requirement: Per-slug pacing limits on per-model providers

For `per-model` providers, proactive pacing (RPM/TPM/RPD) SHALL use separate gate state per
`(credential_id, wire_slug)` pair. Limits SHALL resolve from `provider-limits.yaml` per slug
(explicit model entry or suffix rule) without sharing one daily counter across all slugs on the
credential.

#### Scenario: Nemotron RPD exhaustion does not gate gpt-oss

- **GIVEN** proactive pacing recorded 50 dispatches for `(openrouter-default, nemotron:free)` today
- **WHEN** the router evaluates `(openrouter-default, gpt-oss-120b:free)`
- **THEN** gpt-oss pacing gate is not rejected solely because nemotron reached RPD

---

### Requirement: Acceptance matrix by failure signal

The gateway SHALL ship an architectural test matrix proving per-model domain behavior through
concrete use cases (not provider-named unit stubs only):

| ID | Use case | Expected |
|----|----------|----------|
| A | OR nemotron 429 model-day → gpt-oss 200 same slot | Model scope; no slot retire |
| B | OR 402 paid slug on free-tier account | Model scope; gpt-oss still tried |
| C | Gemini 404 phantom slug | Model scope (regression) |
| D | fast-thinking stability → larger OR model before groq | Intent + ladder |

#### Scenario: Matrix A passes in routing_load

- **WHEN** `routing_load` scenario `openrouter_nemotron_429_then_gpt_oss_200` runs with testing features
- **THEN** the emulator returns 429 for nemotron and 200 for gpt-oss on the same credential
- **AND** the gateway response is 200 from gpt-oss

#### Scenario: Matrix B passes in routing_load

- **WHEN** `routing_load` scenario `openrouter_402_paid_does_not_kill_free` runs
- **THEN** a 402 on a paid slug does not add `openrouter-default` to `failed_credentials`
- **AND** a subsequent `:free` slug on the same credential succeeds
