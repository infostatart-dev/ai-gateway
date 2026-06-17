## MODIFIED Requirements

**Approach:** Two rank axes work together:

1. **Provider default rank** (`default_provider_budget_rank`) — order within cost-class band.
2. **Credential `budget-rank`** in `credentials.yaml` — same provider, different accounts/tiers
   (e.g. eight `gemini-free` at rank 0 vs `gemini-default` paid at rank 10).

**Failover skip predicate** (already in `failover_loop.rs`): on daily quota exhaustion,
skip siblings with **same provider + same credential budget rank** — not all free-tier
providers globally. On HTTP 503 / transient 429, **rotate** to next sibling at same rank.

**Web ordering:** `deepseek-web` before `chatgpt-web`; both after free API keys; paid API
(`anthropic`, `openai`) before web sessions but after free API when available.
`chatgpt-web` is last resort — never attempted while a higher-priority slot succeeds.

**Governance:** providers without API access (see `autodefault-hardening`) are excluded
from rank-0 band entirely — not probed every request.

---

### Requirement: Autodefault provider priority order

The gateway SHALL build autodefault with the following provider priority when credentials
or session files are available (earlier = higher priority within the same cost-class band):

1. `opencode`
2. `openrouter`
3. `github-models`
4. `mistral`
5. `groq`
6. `cerebras`
7. `cloudflare`
8. `gemini`
9. `deepseek-web`
10. `anthropic`
11. `openai`
12. `chatgpt-web`

Curated free expansions (e.g. additional free-API providers) MUST be inserted within the
free-API band — not ahead of `opencode` or behind `chatgpt-web`.

#### Scenario: ChatGPT Web is last resort

**Given** `chatgpt-web` and at least one free API provider are configured
**When** autodefault ranks candidates
**THEN** `chatgpt-web` MUST have the lowest provider priority among configured providers

#### Scenario: DeepSeek Web precedes ChatGPT Web

**Given** both `deepseek-web` and `chatgpt-web` are configured
**When** autodefault ranks candidates
**THEN** `deepseek-web` MUST rank before `chatgpt-web`

#### Scenario: Paid API precedes browser sessions

**Given** paid API providers and a web-session provider are configured
**When** autodefault ranks candidates
**THEN** paid API providers MUST rank before web-session providers
**AND** paid API providers MUST rank after free API providers when those are available

#### Scenario: Provider rank matches priority order

**Given** the provider priority table above
**When** autodefault assigns default provider budget rank
**THEN** earlier providers MUST have lower rank values than later providers

---

### Requirement: Quota-exhausted failover scope

On daily quota exhaustion, the gateway MUST skip remaining candidates that share the
same provider and the same credential budget rank as the failed candidate.

Candidates from other providers or different budget ranks on the same provider MUST remain
in the failover chain.

HTTP 503 overload and transient HTTP 429 rate-limit failures MUST NOT use daily-quota
sibling skip; they MUST rotate to the next sibling slot at the same rank.

#### Scenario: free-tier-quota-skips-siblings-not-paid-tier

**Given** multiple free-tier slots and one paid-tier slot for the same provider
**And** a free-tier slot returns daily quota exhaustion
**When** failover processes the failure
**THEN** remaining free-tier sibling slots MUST be skipped
**AND** the paid-tier slot MUST remain eligible

#### Scenario: single-slot-quota-failover-to-next-provider

**Given** a single credential slot for a provider returns daily quota exhaustion
**And** the next provider in priority order is configured
**When** failover processes the failure
**THEN** the router MUST attempt the next provider
**AND** MUST NOT terminate without trying cross-provider candidates

#### Scenario: overload-rotates-sibling-not-skips-band

**Given** multiple free-tier slots at the same budget rank for one provider
**And** the first slot returns HTTP 503 overload
**When** failover processes the failure
**THEN** the next sibling slot at the same rank MUST be attempted
**AND** MUST NOT skip the entire free-tier band after one 503
