## ADDED Requirements

### Requirement: Gemini free-tier model ladder on one credential

The gateway SHALL define an ordered **model ladder** for Gemini free-tier
credentials. For a single `gemini-free*` credential, failover SHALL walk the
ladder on the **same** credential before advancing to another credential slot.

Default ladder bands (YAML-configurable):

1. **Fast** — `gemini-3-flash-preview`, `gemini-3.5-flash-preview`
2. **Capacity** — `gemini-3.1-flash-lite`, `gemini-2.5-flash`
3. **Stability** — `gemini-2.5-pro` (and optionally other pro slugs when enabled)

Candidates on the same credential SHALL be ordered by ladder position ascending
(fast before capacity before stability) when multiple models are eligible.

#### Scenario: RPD exhaustion on 3-flash tries 3.5 on same slot

- **WHEN** `gemini-free-8` receives per-model daily quota exhaustion for `gemini-3-flash-preview`
- **AND** `gemini-3.5-flash-preview` has remaining quota on the same credential
- **THEN** the gateway retries on `gemini-3.5-flash-preview` without marking `gemini-free-8` dead
- **AND** does not skip to `gemini-free-9` yet

#### Scenario: Capacity band used when fast band exhausted

- **WHEN** all fast-band models on `gemini-free-8` are exhausted or gated out
- **AND** `gemini-3.1-flash-lite` has remaining quota
- **THEN** the gateway dispatches to `gemini-3.1-flash-lite` on the same credential

#### Scenario: Inter-slot failover after full ladder

- **WHEN** every ladder model on `gemini-free-8` is exhausted or in cooldown
- **THEN** the gateway marks the credential slot exhausted for the request walk
- **AND** proceeds to the next configured `gemini-free*` sibling via round-robin

---

### Requirement: Stability escalation within slot

The gateway SHALL support **stability** band failover: when the client request
requires a successful completion and fast/capacity ladder models on a credential
are unavailable, the gateway SHALL attempt stability-band models on the **same**
credential before leaving the slot. Stability models SHALL be larger or more
capable upstream slugs intended to improve answer reliability.

The gateway SHALL NOT select stability-band models as the **first** hop when
fast-band models remain eligible. Stability escalation within the slot SHALL be
**upward** capability for completion—not a downgrade to smaller or flash-only
models for cost saving.

#### Scenario: Stability pro attempted after fast models fail

- **WHEN** fast and capacity models on `gemini-free-8` are exhausted for a request
- **AND** `gemini-2.5-pro` is configured in the stability band and supports request capabilities
- **THEN** the gateway attempts `gemini-2.5-pro` on `gemini-free-8` before `gemini-free-9`

#### Scenario: Fast model preferred when available

- **WHEN** `gemini-3-flash-preview` on `gemini-free-8` is eligible and not gated
- **THEN** the gateway does not start on `gemini-2.5-pro` on that credential

#### Scenario: Json schema required on stability hop

- **WHEN** the request requires strict `json_schema`
- **AND** stability-band model does not advertise json_schema support
- **THEN** that stability candidate is skipped
- **AND** the walk continues to the next ladder or credential slot

---

### Requirement: Per-model failure tracking

The gateway SHALL track failures and cooldown at **model granularity** within a
credential: key `(credential_id, normalized_upstream_model)`. Model-level
transient rate limits SHALL NOT insert the parent credential into the global
`failed_credentials` set for the request walk.

#### Scenario: RPM 429 on one model does not fail credential

- **WHEN** `gemini-3-flash-preview` on `gemini-free-8` returns transient HTTP 429
- **THEN** `(gemini-free-8, gemini-3-flash-preview)` is cooled down or skipped
- **AND** `gemini-free-8` remains eligible for other models in the ladder

#### Scenario: Project billing cap retires whole credential

- **WHEN** upstream response indicates project-wide billing or spending cap exhaustion
- **THEN** the entire `gemini-free-8` credential is marked failed for the walk
- **AND** remaining free Gemini siblings at the same budget-rank may be skipped per existing policy

---

### Requirement: Free-tier catalog alignment for ladder models

The gateway SHALL list every ladder model in embedded `providers.yaml` under
`gemini.models` with conservative capabilities. Embedded `provider-limits.yaml`
SHALL declare per-model RPM/TPM/RPD for each ladder slug matching operator AI
Studio free-tier dashboards (preview flash RPD typically **20**, 3.1 Flash Lite
RPD **500** as of 2026-06).

#### Scenario: Ladder models routable from providers catalog

- **WHEN** embedded `providers.yaml` is loaded
- **THEN** `gemini-3.5-flash-preview` and `gemini-3.1-flash-lite` appear in `gemini.models`
- **AND** each has `supports-json-schema` consistent with stage autodefault needs

---

### Requirement: Tests and observability

CI SHALL cover per-model pacing isolation, intra-slot ladder failover, stability
band ordering, project-cap sibling skip, and unchanged sixteen-slot inter-credential
round-robin. Route trace SHALL include `gemini_ladder_band` and
`gemini_ladder_model` when a Gemini free ladder hop is selected.

#### Scenario: Routing load gemini model ladder scenario

- **WHEN** routing_load scenario `gemini_model_ladder_same_slot` runs
- **THEN** simulated 3-flash RPD exhaustion on one credential is recovered via 3.1-flash-lite on the same credential
- **AND** no inter-slot hop occurs until the ladder is exhausted

#### Scenario: Trace labels ladder hop

- **WHEN** autodefault succeeds via capacity-band model on a Gemini free credential
- **THEN** route trace includes ladder band `capacity` and the upstream model slug
