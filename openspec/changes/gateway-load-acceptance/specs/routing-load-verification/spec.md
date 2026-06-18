## ADDED Requirements

**Proof layer for `gateway-load-acceptance`.** Extends the existing `routing-load-verification`
framework — no second harness. Each scenario proves a gateway guardrail from
`autodefault-hardening`.

---

### Requirement: stage-gap scenario coverage

The routing load framework MUST include concurrent scenarios with these pass invariants:

| Proves | Invariant |
|--------|-----------|
| Hard payload pre-flight | Context-limited provider: zero attempts |
| Daily quota pacing | Exhausted provider: zero re-hits same UTC day |
| Gemini 503 class | Sibling rotation, not skip-all-free |
| ChatGPT last resort | Zero HTTP 413; `upload_parts > 1` when applicable |
| Access-denied cooldown | Zero attempts while slot in long cooldown |

#### Scenario: payload-filter-zero-attempts

**Given** concurrent fat json_schema bodies exceeding an API provider window
**When** the payload-filter scenario completes
**Then** provider-stats MUST show zero attempts for that provider

#### Scenario: daily-quota-zero-rehits

**Given** daily quota exhausted mid-scenario
**When** subsequent concurrent requests arrive before daily reset
**Then** provider-stats MUST show zero new attempts for that provider

#### Scenario: gemini-503-sibling-rotation

**Given** first free slot returns HTTP 503 and second succeeds
**When** concurrent failover runs
**Then** terminal success MUST land on the sibling credential

---

### Requirement: optional emulator-backed HTTP path

Documentation MUST describe running Level 2 harness scenarios against `mise dev:emulated` as
the HTTP complement to in-process tests. This path is optional for CI; in-process scenarios
are the required gate.

#### Scenario: harness-runs-without-live-keys

**Given** gateway and emulator use embedded catalogs with synthetic secrets
**When** harness scenarios execute against the emulated stack
**Then** provider-stats assertions MUST pass without live provider API keys
