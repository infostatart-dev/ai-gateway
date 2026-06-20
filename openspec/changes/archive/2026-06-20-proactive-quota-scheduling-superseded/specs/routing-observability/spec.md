## ADDED Requirements

### Requirement: Provider-stats exposes quota oracle state per credential

`GET /v1/observability/provider-stats` routing rows SHALL include for each
credential (when quota oracle data exists):

- `next_available_at` — RFC3339 timestamp or `null` when callable
- `blocked_reason` — one of `none`, `rpm`, `rpd`, `tpm`, `cooldown`, `circuit`, `upstream_reset`

#### Scenario: RPM-blocked slot shows next_available_at

- **WHEN** `gemini-free-2` has RPM wait 45s for the preferred model
- **THEN** provider-stats row for `gemini-free-2` includes `blocked_reason=rpm`
- **AND** `next_available_at` is approximately 45s in the future

#### Scenario: Callable slot shows null next_available_at

- **WHEN** a credential has no blocking sources
- **THEN** `blocked_reason` is `none`
- **AND** `next_available_at` is `null`

---

### Requirement: Repeat 429 violations exposed in observability

The gateway SHALL expose `repeat_429_violations` (count since process start) on the
provider-stats snapshot root and `repeat_429_violation` (boolean) per hop on route trace.

#### Scenario: Clean run shows zero violations

- **WHEN** no oracle-blocked pair receives upstream 429
- **THEN** provider-stats `repeat_429_violations` is 0

#### Scenario: Violation increments snapshot counter

- **WHEN** a repeat 429 violation occurs
- **THEN** provider-stats `repeat_429_violations` increases by 1
