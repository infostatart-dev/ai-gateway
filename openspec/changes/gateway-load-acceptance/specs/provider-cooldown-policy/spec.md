## ADDED Requirements

**Approach:** Cooldown is **per credential account** (slot), not per provider brand.
`ProviderLimitCatalog::cooldown_for(provider)` supplies provider defaults; each slot
maintains its own `cooldown_until` in router state.

**Not midnight-by-default:** cooldown duration comes from upstream hints and catalog
overrides per failure class — different providers already declare different values
(e.g. browser-session rate-limit 180s vs 120s). Daily quota exhaustion uses the provider's
`quota-exhausted` override (e.g. 24h), not a short `provider-error` cooldown.

**Pairs with:** `catalog-quota-pacing` (proactive reject) + `autodefault-hardening` (access-denied
24h for providers without API access).

---

### Requirement: per-provider cooldown from catalog

Cooldown duration for a failure class MUST be resolved from the provider limit catalog:

1. Upstream `Retry-After` header or JSON reset hint (`retryDelay`, `try again in …`) —
   highest priority.
2. Provider-level `cooldown` override for the failure class:
   `rate-limit`, `quota-exhausted`, `provider-error`, `auth-error`, `abuse-block`.
3. Global `cooldown-defaults` — fallback only when no provider override exists.

#### Scenario: browser-session-rate-limit-uses-provider-override

**Given** a browser-session provider declares a rate-limit cooldown longer than the global default
**And** upstream returns HTTP 429 classified as transient rate limit with no reset hint
**When** cooldown is computed
**Then** the cooldown MUST use that provider's rate-limit override
**And** MUST NOT use the global rate-limit default

#### Scenario: upstream-retry-after-takes-precedence

**Given** upstream returns HTTP 429 with `Retry-After: 45` or a JSON retry delay
**When** cooldown is computed for any provider
**Then** the upstream hint MUST be used regardless of catalog defaults

#### Scenario: quota-exhausted-uses-provider-override-not-global-only

**Given** a provider declares `quota-exhausted: 24h` in its cooldown block
**And** upstream returns daily quota exhaustion with no reset hint
**When** cooldown is computed
**Then** the cooldown MUST use the 24h provider override (plus buffer)
**And** MUST NOT apply only the global 1h `quota-exhausted` default

---

### Requirement: per-credential-slot cooldown state

Cooldown state MUST be tracked per credential slot, not per provider globally.
Two slots for the same provider MUST have independent cooldown timelines.

`effective_budget_rank` MAY deprioritize slots with short remaining cooldown without
removing them from the candidate list entirely.

#### Scenario: independent-cooldown-per-sibling-slot

**Given** one multi-slot free-tier credential returns daily quota exhaustion and enters cooldown
**When** a sibling slot for the same provider is evaluated for the next request
**Then** the sibling slot MUST be eligible unless it has its own active cooldown
**And** MUST NOT inherit the failed slot's cooldown state

#### Scenario: access-denied-long-cooldown-per-slot

**Given** a credential slot enters an access-denied cooldown (unsupported model / auth failure)
**When** autodefault evaluates candidates within the cooldown window
**Then** only that slot MUST be omitted
**And** other providers MUST remain eligible without waiting for the denied slot's cooldown
