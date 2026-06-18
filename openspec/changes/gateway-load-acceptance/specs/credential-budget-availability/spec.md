## ADDED Requirements

**Serves:** autodefault-hardening item 8 — OpenRouter (and extensible providers) need
**credit state** plus **catalog RPM/RPD**; zero balance does not disable free-tier routes.

Uses existing `runtime-sources` declarations in `provider-limits.yaml` (OpenRouter `key-info`,
OpenAI `response-headers`). New module: `router/budget_probe/`.

---

### Requirement: catalog-driven budget probe

The gateway MUST poll or update budget state from `runtime-sources` per credential slot.
Background interval default 5 minutes; OpenAI-style providers MAY update from response headers
after each dispatch.

#### Scenario: key-info-polled-per-slot

**Given** a slot has a key-info runtime source configured
**When** the gateway starts
**Then** a probe task MUST populate per-slot budget state from that endpoint

---

### Requirement: dual gate credits and catalog quotas

Dispatch eligibility MUST satisfy **both**:

1. Catalog pacing permits (RPM/TPM/RPD) — see `catalog-quota-pacing`.
2. Budget probe does not block the requested route (paid vs free).

Zero credits MUST NOT blanket-disable a slot when free-tier catalog limits still apply
and the request maps to a `:free` model variant.

#### Scenario: zero-credits-free-tier-routable

**Given** zero remaining credits and free-tier eligibility
**And** the request targets a `:free` model
**When** catalog pacing permits
**Then** the slot MUST remain dispatchable

#### Scenario: zero-credits-paid-blocked

**Given** zero remaining credits
**And** the request requires a paid model route
**When** the router evaluates the slot
**Then** the slot MUST be skipped without upstream HTTP

---

### Requirement: probe failure fail-open

Probe HTTP failures MUST retain the last snapshot and MUST NOT block dispatch solely due
to probe error.

#### Scenario: timeout-retains-snapshot

**Given** the previous probe succeeded
**When** the next probe times out
**Then** dispatch MUST use the last known snapshot
