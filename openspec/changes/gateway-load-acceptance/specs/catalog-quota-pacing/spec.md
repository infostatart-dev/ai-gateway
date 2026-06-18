## ADDED Requirements

**Serves:** autodefault-hardening item 6 — stop daily-cap providers from re-hit every request.

Extends existing `PacingGate` (today: RPM + concurrent only) with TPM minute window and
RPD/TPD daily counters per credential scope. Same limit resolution path as
`per_request_token_cap` in payload filtering.

---

### Requirement: multi-dimension pacing limits

`PacingLimits` MUST expose RPM, TPM, RPD, and TPD from the provider limit catalog for the
resolved `(provider, tier, model)`. Absent dimensions MUST NOT block dispatch.

#### Scenario: all-defined-dimensions-resolved

**Given** a tier defines RPM, TPM, and RPD in the catalog
**When** pacing limits are built for that scope
**Then** all three dimensions MUST be populated on `PacingLimits`

---

### Requirement: proactive reject before upstream

The pacing gate MUST reject when any defined dimension is exhausted, without an upstream
HTTP call. Retry-after MUST align to the dimension boundary (minute edge for RPM/TPM;
`daily-reset-utc-hour` for RPD/TPD).

#### Scenario: daily-exhausted-no-upstream-hop

**Given** RPD counter reached the catalog limit for a scope
**When** a permit is requested
**Then** the gate MUST reject immediately
**And** failover MUST advance to the next ranked provider

#### Scenario: tpm-minute-window

**Given** TPM usage in the current minute plus estimated request tokens would exceed the limit
**When** a permit is requested
**Then** the gate MUST reject until the minute window rolls over
