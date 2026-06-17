## MODIFIED Requirements

**Approach:** The emulator is the **universal catalog-driven upstream** — one YAML catalog
drives limits, TTFB, capabilities, and injectable failure profiles. Gateway and emulator
share `provider-limits.yaml` + `credentials.yaml`; divergence is a bug.

**Verification stack** (no live keys):

```
routing_load (in-process, per-credential mocks)
        ↓
mise dev:emulated (HTTP, synthetic secrets)
        ↓
k6 soak optional (poll provider-stats)
```

**Admin control plane:** `/_admin/reset`, `/_admin/state`, forced response profiles
(429-rpm, 429-quota, 503-overload, 402, 400-context) for deterministic failover tests.

Production gateway learns **real** TTFB via observability; catalog `expected-ttfb-ms` is
emulator-only baseline.

---

### Requirement: catalog-driven latency

The upstream emulator MUST read expected latency from the provider limit catalog only.

Each provider entry MAY declare:

```yaml
expected-ttfb-ms: 320
ms-per-token: 0.05
daily-reset-utc-hour: 0
```

Delay formula: `delay_ms = expected_ttfb_ms + completion_tokens × ms_per_token`

Hardcoded latency tables in Rust MUST NOT be the source of truth.

#### Scenario: emulator-uses-catalog-ttfb

**Given** the catalog sets `expected-ttfb-ms: 999` for a provider
**When** the emulator handles a request for that provider
**Then** response delay MUST use 999ms as base plus token component

#### Scenario: no-separate-latency-config

**Given** the emulator starts without external latency override
**When** time-to-first-byte is computed
**Then** values MUST come from the embedded provider limit catalog only

---

### Requirement: catalog-symmetric quota enforcement

The emulator MUST enforce RPM, TPM, RPD, and TPD using the **identical** limit resolution
path as gateway pacing. No emulator-specific limit file.

#### Scenario: admin-state-matches-catalog-limits

**Given** the emulator starts with standard embedded catalogs
**When** admin state is queried
**Then** each active credential scope MUST show catalog limit values
**And** scopes with catalog limits MUST NOT show all-null limits

#### Scenario: json-429-on-quota-hit

**Given** a scope reaches RPM or RPD limit
**When** the next request targets that scope
**Then** the emulator MUST return HTTP 429 with JSON body parseable by gateway retry logic
**And** MUST include `Retry-After` when applicable

#### Scenario: json-503-on-overload-profile

**Given** admin forces overload profile on a scope
**When** the gateway dispatches to that scope
**Then** the emulator MUST return HTTP 503 with overload JSON
**And** the gateway MUST classify it for sibling rotation (not daily-quota skip)

#### Scenario: json-402-on-insufficient-credits

**Given** admin forces insufficient-credits profile
**When** the gateway dispatches a paid-model route
**Then** the emulator MUST return HTTP 402 with parseable JSON

---

### Requirement: capability-faithful responses

The emulator MUST enforce model capabilities from the catalog. Unsupported structured
output requests MUST return HTTP 422. Context overflow beyond the catalog-declared window
MUST return HTTP 400 with an OpenAI-compatible context length error.

#### Scenario: unsupported-structured-output-returns-422

**Given** the model does not support structured output in the catalog
**When** the request includes JSON schema response format
**Then** the emulator MUST return HTTP 422

#### Scenario: context-overflow-returns-400

**Given** estimated input exceeds catalog context window
**When** the emulator receives the request
**Then** the emulator MUST return HTTP 400 context length error

---

### Requirement: autodefault verification without live upstream

All tunable emulator behaviour MUST originate from embedded catalogs. The emulator MUST
be sufficient to validate autodefault routing, pacing, cooldown, and failover without
live API keys.

#### Scenario: emulated-responses-match-catalog-limits

**Given** the gateway routes all providers to the emulator
**When** autodefault verification runs
**Then** quota, credit, and context responses MUST match catalog limits per scope
**And** TTFB MUST reflect `expected-ttfb-ms` from the catalog
