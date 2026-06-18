## ADDED Requirements

### Requirement: Unpaid route 402 on per-model profile

The gateway SHALL classify HTTP 402 on non-`:free` wire slugs as `ExhaustionScope::Model` when the
provider has `quota-profile: per-model` and the body indicates the account never purchased credits
or has insufficient credits for that paid route.
The gateway SHALL retire only `(credential_id, wire_slug)` for the request walk and SHALL NOT add
the credential to `failed_credentials` solely because one unpaid slug returned 402. Project-scoped
402/403/429 billing-cap patterns SHALL continue to map to `ExhaustionScope::Project`.

#### Scenario: OpenRouter paid slug 402 does not kill free siblings

- **GIVEN** provider `openrouter` has `quota-profile: per-model`
- **WHEN** upstream returns HTTP 402 for wire slug `openai/gpt-4o-mini` on `openrouter-default`
- **AND** body contains `never purchased credits`
- **THEN** `ExhaustionScope` is `Model`
- **AND** `openai/gpt-oss-120b:free` on `openrouter-default` remains eligible in the same request

#### Scenario: Billing cap remains project scope

- **WHEN** upstream 402 body matches project billing cap patterns
- **THEN** `ExhaustionScope` is `Project`

---

### Requirement: OpenRouter free-models-per-day classification

The gateway SHALL classify HTTP 429 bodies containing `free-models-per-day` as
`FailoverClass::QuotaExhausted` (not transient RPM). Cooldown duration SHALL prefer
`X-RateLimit-Reset` from response headers when present (epoch milliseconds), otherwise provider
`quota-exhausted` override.

#### Scenario: free-models-per-day uses reset header

- **WHEN** upstream 429 body contains `free-models-per-day`
- **AND** response header `X-RateLimit-Reset` is present
- **THEN** model cooldown for `(credential, slug)` extends until that reset instant
- **AND** `FailoverClass` is `QuotaExhausted`

#### Scenario: Classifier unit test

- **WHEN** `classify_429` runs on OpenRouter nemotron 429 body
- **THEN** result is `QuotaExhausted`

## MODIFIED Requirements

### Requirement: Quota-profile-aware exhaustion scope

`ExhaustionScope` classification SHALL consider the provider's
`ProviderQuotaProfile` from the embedded limit catalog:

| Profile | 404 NOT_FOUND | 400 unsupported model | 429 RPM | 429 model RPD | 429 billing | 402 unpaid slug | 503 high demand |
|---------|---------------|----------------------|---------|---------------|-------------|-----------------|-----------------|
| `per-model` | Model | Model | Model | Model | Project | Model | Slot |
| `per-slot` | Slot | Slot | Slot | Project | Project | Project | Slot |
| `per-session` | Slot | Slot | Slot | Slot | Project | Project | Slot |

#### Scenario: Per-model 404 retires slug only

- **GIVEN** provider `gemini` has `quota-profile: per-model`
- **WHEN** upstream returns HTTP 404 for `gemini-3.5-flash-preview` on `gemini-free-8`
- **THEN** `ExhaustionScope` is `Model`
- **AND** `failed_models` contains `(gemini-free-8, gemini-3.5-flash-preview)`
- **AND** `failed_credentials` does not contain `gemini-free-8`

#### Scenario: Per-slot 404 retires credential

- **GIVEN** a provider with `quota-profile: per-slot`
- **WHEN** upstream returns HTTP 404 for a model on credential `slot-a`
- **THEN** `ExhaustionScope` is `Slot`
- **AND** `failed_credentials` contains `slot-a`

#### Scenario: Unsupported model body on per-model provider

- **GIVEN** provider `gemini` has `quota-profile: per-model`
- **WHEN** upstream returns HTTP 400 with body containing `unsupported model`
- **THEN** `ExhaustionScope` is `Model`

#### Scenario: Per-model unpaid 402 retires slug only

- **GIVEN** provider `openrouter` has `quota-profile: per-model`
- **WHEN** upstream returns HTTP 402 for a non-`:free` slug on `openrouter-default`
- **THEN** `ExhaustionScope` is `Model`
