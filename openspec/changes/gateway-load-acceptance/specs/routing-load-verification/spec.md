## ADDED Requirements

**Cross-change alignment:** `gateway-load-acceptance` production fixes MUST be provable
through the existing `routing-load-verification` framework. This delta adds stage-gap
scenarios — not a second test harness.

---

### Requirement: stage-gap scenario coverage

The routing load verification framework MUST include scenarios that exercise
`gateway-load-acceptance` guardrails under concurrent dispatch:

| Scenario concern | Layer under test | Pass invariant |
|-----------------|------------------|----------------|
| Fat json_schema payload | Payload pre-filter | Context-limited API providers: zero attempts |
| Daily quota exhaustion | Pacing + failover | Exhausted provider: zero re-hops same UTC day |
| HTTP 503 overload | Failover class | Sibling rotation, not skip-all-free |
| ChatGPT last resort + fat body | Web chunking | Zero HTTP 413; `upload_parts > 1` when applicable |
| Access-denied provider | Cooldown governance | Zero attempts while in long cooldown |

#### Scenario: payload-filter-zero-attempts-for-overflow-provider

**Given** concurrent requests with fat json_schema bodies exceeding an API provider window
**When** the payload-filter scenario completes
**Then** provider-stats MUST show zero attempts for that provider's credentials
**And** eligible providers MUST show positive attempts

#### Scenario: daily-quota-zero-rehits

**Given** a provider's daily quota is exhausted mid-scenario
**When** subsequent concurrent requests arrive before daily reset
**Then** provider-stats MUST show zero new attempts for that provider's credentials
**And** failover MUST route to other providers

#### Scenario: gemini-503-sibling-rotation-under-load

**Given** per-credential mocks return HTTP 503 on the first free slot and success on the second
**When** concurrent failover scenario runs
**Then** terminal success MUST land on the sibling credential
**And** browser-session last-resort credentials MUST show zero attempts

---

### Requirement: emulator-backed verification path

Routing load verification MUST document execution against the catalog-driven upstream
emulator (`mise dev:emulated`) as the HTTP-level complement to in-process Level 1/2 tests.

#### Scenario: harness-runs-against-emulator

**Given** gateway and emulator both use embedded catalogs with synthetic secrets
**When** Level 2 harness scenarios execute against the emulated stack
**Then** provider-stats assertions MUST pass without live provider API keys
