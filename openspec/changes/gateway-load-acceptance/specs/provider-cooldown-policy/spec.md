## ADDED Requirements

**Serves:** autodefault-hardening item 7 — cooldown is per **credential slot**, duration from
**provider catalog** per failure class, not a global constant.

Stack: upstream hint → provider `cooldown` override → `cooldown-defaults` fallback.
`ProviderLimitCatalog::cooldown_for(provider)` already merges these; fix is using provider
`quota-exhausted` override in `resolve_429_base_secs`, not only global 1h.

---

### Requirement: cooldown resolution stack

Cooldown duration MUST be resolved in this order:

1. Upstream `Retry-After` or JSON reset hint.
2. Provider-level override for the failure class in the catalog.
3. Global `cooldown-defaults`.

#### Scenario: provider-rate-limit-override

**Given** a provider declares a longer rate-limit cooldown than the global default
**And** no upstream hint is present
**When** transient rate-limit cooldown is computed
**Then** the provider override MUST be used

#### Scenario: quota-exhausted-provider-override

**Given** a provider declares `quota-exhausted: 24h`
**And** daily quota exhaustion with no upstream hint
**When** cooldown is computed
**Then** the 24h provider override MUST be used

---

### Requirement: independent cooldown per credential slot

Each credential slot MUST maintain its own `cooldown_until`. Sibling slots MUST NOT
inherit another slot's cooldown.

#### Scenario: sibling-not-inherits-cooldown

**Given** slot A entered cooldown after a failure
**When** slot B for the same provider is evaluated
**Then** slot B MUST be eligible unless B has its own active cooldown
