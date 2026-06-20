## ADDED Requirements

### Requirement: Reconcile block uses upstream truth not router constants

The gateway MUST derive reconcile `until` instants from upstream response signals (headers, body
classifiers) before applying catalog `cooldown-defaults` fallbacks. Router code SHALL NOT embed
fixed block durations for provider/account/model exhaustion.

#### Scenario: Retry-After drives reconcile on RPM 429

- **WHEN** upstream returns 429 with `Retry-After: 45`
- **THEN** reconcile blocks the classified `PacingScope` until now + 45s
- **AND** catalog `rate-limit: 60s` is not used

#### Scenario: Fallback cooldown when upstream silent

- **WHEN** upstream returns 429 without reset headers or parseable body
- **THEN** reconcile uses merged `cooldown-defaults` from `provider-limits.yaml`

---

### Requirement: Exhaustion scope determines reconcile granularity

Reconcile SHALL apply at the scope matching `ExhaustionScope`:

| Scope | Reconcile target |
|-------|------------------|
| `Model` | `CredentialModel` pacing scope + model cooldown |
| `Slot` | `Credential` or `Session` pacing scope + slot cooldown |
| `Project` | provider-level sibling skip per existing rules |

#### Scenario: Per-model 429 reconciles slug only

- **WHEN** `ExhaustionScope::Model` for `(gemini-free-8, gemini-3-flash-preview)`
- **THEN** reconcile blocks only that `CredentialModel` scope
- **AND** other models on `gemini-free-8` remain feasible if admission allows
