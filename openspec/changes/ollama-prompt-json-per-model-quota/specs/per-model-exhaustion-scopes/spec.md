## MODIFIED Requirements

### Requirement: Quota-profile-aware exhaustion scope

`ExhaustionScope` classification SHALL consider the provider's
`ProviderQuotaProfile` from the embedded limit catalog:

| Profile | 404 NOT_FOUND | 400 unsupported model | 429 RPM | 429 model RPD | 429 billing | 403 subscription required | 503 high demand |
|---------|---------------|----------------------|---------|---------------|-------------|---------------------------|-----------------|
| `per-model` | Model | Model | Model | Model | Project | Model | Slot |
| `per-slot` | Slot | Slot | Slot | Project | Project | Slot | Slot |
| `per-session` | Slot | Slot | Slot | Slot | Project | Slot | Slot |

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

#### Scenario: Ollama Cloud subscription 403 retires slug only

- **GIVEN** provider `ollama-cloud` has `quota-profile: per-model`
- **WHEN** upstream returns HTTP 403 with body indicating subscription or plan required for `kimi-k2.6`
- **THEN** `ExhaustionScope` is `Model`
- **AND** `failed_models` contains `(credential, kimi-k2.6)`
- **AND** free-tier slugs on the same credential remain eligible
