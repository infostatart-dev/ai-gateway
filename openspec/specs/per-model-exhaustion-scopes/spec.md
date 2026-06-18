# per-model-exhaustion-scopes

## Purpose

Classify upstream exhaustion (404, 429, 503, billing) using each provider's
`quota-profile` so per-model providers retire `(credential, model)` pairs instead
of whole free slots when one ladder slug fails.

## Requirements

### Requirement: Quota-profile-aware exhaustion scope

`ExhaustionScope` classification SHALL consider the provider's
`ProviderQuotaProfile` from the embedded limit catalog:

| Profile | 404 NOT_FOUND | 400 unsupported model | 429 RPM | 429 model RPD | 429 billing | 503 high demand |
|---------|---------------|----------------------|---------|---------------|-------------|-----------------|
| `per-model` | Model | Model | Model | Model | Project | Slot |
| `per-slot` | Slot | Slot | Slot | Project | Project | Slot |
| `per-session` | Slot | Slot | Slot | Slot | Project | Slot |

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

---

### Requirement: Model-level cooldown for phantom slugs

The gateway SHALL apply a long model cooldown (duration from provider catalog
override or default ≥ 1h) when `ExhaustionScope::Model` results from HTTP 404 or
unsupported-model HTTP 400 on a `per-model` provider, so phantom slugs are not
retried on every request.

#### Scenario: 404 model enters long cooldown

- **WHEN** `gemini-3.5-flash-preview` returns 404 on `gemini-free-8`
- **THEN** model state for `(gemini-free-8, gemini-3.5-flash-preview)` has active cooldown
- **AND** other models on `gemini-free-8` remain eligible

---

### Requirement: 503 high demand slot cooldown on per-model providers

The gateway SHALL, when a `per-model` provider returns HTTP 503 with a body
indicating high demand or temporary overload: (1) apply short slot-level cooldown
on the credential (30–60s from catalog `high-demand` override or `provider-error`
default); (2) continue the intra-slot ladder on the current request walk when
other ladder models on the same credential remain available; (3) NOT skip all
free Gemini siblings solely because of one 503 on one model.

#### Scenario: 503 on 3.5 flash cools slot but continues ladder

- **GIVEN** `gemini-free-8` receives 503 high demand on `gemini-3.5-flash`
- **AND** `gemini-3.1-flash-lite` is the next ladder model on the same credential
- **THEN** the gateway attempts `gemini-3.1-flash-lite` on `gemini-free-8` in the same request walk
- **AND** `gemini-free-8` pacing gate receives a short slot cooldown for subsequent requests

#### Scenario: 503 body classifier

- **WHEN** upstream 503 body contains "high demand"
- **THEN** `looks_like_high_demand` returns true
- **AND** scope classification uses the 503 high-demand path for per-model providers

---

### Requirement: Project billing unchanged

HTTP 402/403/429 with project billing cap patterns SHALL continue to map to
`ExhaustionScope::Project` regardless of quota profile.

#### Scenario: Billing cap still skips siblings

- **WHEN** upstream 429 body matches project billing cap patterns
- **THEN** `ExhaustionScope` is `Project`
- **AND** free sibling skip behavior applies as before

---

### Requirement: Scope classification tests

The gateway SHALL ship unit tests for `classify_exhaustion_scope` covering at
minimum: per-model 404→Model, per-model 503→Slot, per-slot 404→Slot, per-model
429 RPM→Model, project billing→Project.

#### Scenario: Unit test matrix passes in CI

- **WHEN** `cargo test quota_scope` runs with all features
- **THEN** per-model 404 and 503 cases pass without live API keys
