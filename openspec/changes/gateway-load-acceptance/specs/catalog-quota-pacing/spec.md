## ADDED Requirements

**Approach:** Pacing is the **proactive** gate — it must know RPM, TPM, and RPD/TPD *before*
HTTP. Reactive 429 handling stays in cooldown/failover; pacing prevents the dead-hop loop
where the router discovers limits only after burning a slot (stage pattern: daily-cap
providers re-hit every request).

**Layering** (aligned with routing-load-verification): decision shaper limits concurrent
free-tier acquisitions; router ranks and fails over; **pacing enforces catalog quotas per
credential scope**. Do not conflate shaper rejection with router imbalance — separate
scenarios exercise each layer.

**Resolution path:** `(credential slot → provider + tier → model)` — identical to emulator
`resolve_limits` and payload `per_request_token_cap`.

---

### Requirement: multi-dimension pacing limits

Pacing limits MUST expose all quota dimensions defined in the provider limit catalog for
the resolved credential scope: RPM, TPM, RPD, and TPD (each optional when absent).

Limits MUST be resolved from provider, tier, and model — using the same resolution rules
as the upstream emulator and the budget-aware router.

#### Scenario: rpm-tpm-rpd-resolved-from-catalog

**Given** a tier defines RPM 30, TPM 60000, and RPD 7000
**When** pacing limits are built for that tier
**Then** RPM MUST be 30
**And** TPM MUST be 60000
**And** RPD MUST be 7000

#### Scenario: absent-dimension-is-unlimited

**Given** a tier defines RPM 10 but no RPD field
**When** pacing limits are built for that scope
**Then** RPD MUST be unset
**And** the RPD gate MUST NOT block dispatch for that scope

---

### Requirement: proactive scope quota gates

The pacing gate MUST maintain per-credential-scope counters for each defined quota
dimension before upstream dispatch:

- **RPM**: sliding 60-second window (existing `RpmWindow`).
- **TPM**: estimated input tokens from the request body per minute window (reuse
  `token_estimate` — same signal as payload-aware routing).
- **RPD / TPD**: daily counter reset at the provider's `daily-reset-utc-hour` in the
  catalog (default 0).

When a dimension is exhausted, the gate MUST reject immediately without an upstream call
and MUST return a retry-after aligned to the dimension:

| Dimension | Retry-after boundary |
|-----------|---------------------|
| RPM / TPM | Next minute window edge |
| RPD / TPD | Next daily reset hour |

#### Scenario: rpd-exhausted-scope-rejected-before-dispatch

**Given** a credential scope has reached its RPD limit
**When** the pacing gate is asked for a permit
**Then** the gate MUST reject without dispatching upstream
**And** MUST return a retry-after until the provider's daily reset hour

#### Scenario: tpm-window-blocks-oversized-minute

**Given** a scope has TPM usage near its minute limit
**And** the incoming request estimates prompt tokens that would exceed the remaining TPM budget
**When** the pacing gate evaluates TPM
**Then** the gate MUST reject until the minute window rolls over

#### Scenario: daily-counter-resets-at-configured-hour

**Given** a provider declares a daily reset UTC hour
**When** that reset hour arrives
**Then** RPD and TPD usage for all scopes of that provider MUST reset to zero

#### Scenario: daily-cap-provider-zero-rehits-after-first-exhaustion

**Given** a provider with a small daily allocation (neurons or token quota) has exhausted RPD/TPD
**When** subsequent autodefault requests arrive before daily reset
**Then** the pacing gate MUST reject every request for that scope without upstream HTTP
**And** failover MUST advance to the next ranked provider immediately
