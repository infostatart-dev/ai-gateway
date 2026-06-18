## MODIFIED Requirements

**Verification vehicle only.** The emulator lets `routing_load` and `mise dev:emulated` prove
gateway behaviour without live API keys. It is NOT a separate product; it MUST consume the
**same** `provider-limits.yaml` and credential catalog as production routing.

No separate emulator limit file. No hardcoded latency table as source of truth.

---

### Requirement: catalog-symmetric rate limits

The emulator MUST enforce RPM, TPM, RPD, and TPD using the same catalog resolution as the
gateway pacing layer. `/_admin/state` MUST reflect active counters for the configured scope.

#### Scenario: rpm-enforced-without-gateway

**Given** catalog RPM for a provider tier
**When** requests exceed that RPM against the emulator directly
**Then** the emulator MUST return HTTP 429 before the gateway would need to pace

---

### Requirement: catalog-driven latency

Time-to-first-byte MUST come from `expected-ttfb-ms` (and optional `ms-per-token`) in the
provider limit catalog. Editing the catalog MUST change emulator latency without code changes.

#### Scenario: ttfb-from-catalog-field

**Given** `expected-ttfb-ms` is set for a provider in the catalog
**When** a successful completion is emitted
**Then** observed TTFB MUST be within tolerance of that configured value

---

### Requirement: capability-faithful error surfaces

The emulator MUST honour model capabilities from the embedded catalog (context window,
structured output) and MAY expose admin force profiles (`429-rpm`, `429-quota`, `503-overload`,
`402`, `400-context`) for deterministic failure injection during verification.

#### Scenario: context-window-400

**Given** a request exceeds the catalog context window for a model
**When** the emulator processes the request
**Then** it MUST return HTTP 400 consistent with that provider family
