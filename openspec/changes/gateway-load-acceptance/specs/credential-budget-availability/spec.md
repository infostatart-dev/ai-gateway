## ADDED Requirements

**Approach:** Universal **credential budget probe** — one abstraction over `runtime-sources`
in the provider limit catalog. A slot is dispatchable only when **both**:

1. **Catalog pacing** permits (RPM / TPM / RPD) — see `catalog-quota-pacing`.
2. **Budget probe** does not report a hard block for the requested route (paid vs free).

Credit balance and rate limits are **orthogonal**. Example: OpenRouter with zero purchased
credits still has catalog `rpm` / `rpd` on `:free` model variants — the slot remains usable
on the free path under pacing counters.

**Provider examples (v1):**

| Provider | Runtime source | Pre-dispatch signal | Post-dispatch feedback |
|----------|---------------|---------------------|------------------------|
| OpenRouter | `key-info` | `limit-remaining`, `is-free-tier` | 402 → refresh + cooldown |
| OpenAI | `response-headers` | — | `x-ratelimit-remaining-*` after each call |
| Gemini | 429 body `RetryInfo` | — | reactive (future header source) |

Probe failures MUST fail-open (retain last snapshot).

---

### Requirement: runtime budget probe from catalog

The gateway MUST probe runtime budget state using `runtime-sources` entries declared per
provider in the provider limit catalog.

Each credential slot with a configured runtime source MUST be probed on a background
interval (default 5 minutes) and updated after responses that carry rate-limit or credit
headers.

#### Scenario: key-info-polled-per-slot

**Given** a credential slot is configured with a key-info runtime source
**When** the gateway starts
**Then** a background probe MUST poll the key-info endpoint for that slot
**And** MUST store the parsed snapshot in per-slot budget state

#### Scenario: response-headers-update-after-dispatch

**Given** an upstream response includes rate-limit remaining headers
**When** the dispatch completes
**Then** the gateway MUST update that slot's budget snapshot from the response headers
**And** MUST fold remaining counts into subsequent pacing decisions where applicable

---

### Requirement: dual gate paid path vs free path

When a credit-based provider reports zero remaining credits, the slot MUST NOT be
blanket-disabled if the catalog still defines RPM, RPD, or TPM limits for the active tier
and the request maps to a free-tier model variant.

A hard pre-dispatch block applies when:

- budget probe reports no remaining credits on the paid path, **and**
- the mapped upstream model requires paid credits (no `:free` suffix / free-tier route).

Reactive HTTP 402 MUST refresh the probe snapshot, apply slot cooldown, and failover to
`:free` variant or next provider — not repeat 402 on the same paid route in one chain.

#### Scenario: zero-credits-free-tier-still-routable

**Given** a provider reports zero remaining credits and free-tier eligibility
**And** the request targets a free-tier model variant
**When** catalog RPM and RPD counters permit the request
**Then** the slot MUST remain dispatchable
**And** MUST NOT be skipped solely because credit balance is zero

#### Scenario: zero-credits-paid-route-blocked

**Given** a provider reports zero remaining credits
**And** the request targets a paid model route
**When** the router evaluates the slot
**Then** the slot MUST be skipped without an upstream call
**And** failover MUST advance to the next ranked candidate

#### Scenario: reactive-402-refreshes-probe-and-failover

**Given** upstream returns HTTP 402 insufficient credits on a paid route
**When** failover processes the failure
**Then** the budget probe snapshot for that slot MUST be refreshed
**And** the next hop MUST be a free-tier variant or a different provider

---

### Requirement: budget probe fail-open

When a runtime budget probe call fails due to timeout or server error, the gateway MUST
retain the last known snapshot, log a warning, and MUST NOT block dispatch solely due to
probe failure.

#### Scenario: probe-timeout-retains-last-snapshot

**Given** the previous budget probe succeeded with a positive remaining balance
**When** the next probe times out
**Then** dispatch decisions MUST use the last known snapshot
**And** MUST NOT treat the slot as exhausted
