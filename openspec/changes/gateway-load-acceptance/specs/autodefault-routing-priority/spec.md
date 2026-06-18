## MODIFIED Requirements

**Serves:** autodefault-hardening items 5 and 10 — provider **priority order** and **failover
scope** (provider ≠ account; free-tier siblings ≠ all free providers).

---

### Requirement: autodefault provider priority order

`credential_budget_rank` MUST order providers for autodefault:

1. opencode
2. openrouter
3. github-models
4. mistral
5. groq
6. cerebras
7. cloudflare
8. gemini
9. deepseek-web
10. anthropic
11. openai
12. chatgpt-web (last resort)

Access-denied or long-cooldown providers MUST rank after all eligible providers.

#### Scenario: chatgpt-web-last-resort

**Given** at least one API-key provider is eligible
**When** autodefault ranks candidates
**Then** chatgpt-web MUST rank below every eligible API-key provider

#### Scenario: deepseek-before-chatgpt

**Given** both deepseek-web and chatgpt-web are eligible
**When** autodefault ranks candidates
**Then** deepseek-web MUST rank above chatgpt-web

---

### Requirement: failover scope by failure class

Failover skip scope MUST depend on failure class:

| Failure class | Skip scope |
|---------------|------------|
| Transient (429 RPM, 503 overload) | Current slot only; try siblings at same rank |
| QuotaExhausted (daily cap) | Same provider + same `credential_budget_rank` |
| AccessDenied / unsupported model | That credential slot until cooldown expires |

Cross-provider failover MUST remain when all same-rank siblings are exhausted.

#### Scenario: quota-skip-same-rank-siblings-only

**Given** a free-tier slot returns daily quota exhaustion
**When** failover evaluates skip set
**Then** other free-tier siblings at the same rank MUST be skipped
**And** paid-tier slots at a different rank MUST remain eligible

#### Scenario: overload-try-next-sibling

**Given** HTTP 503 overload on the first free-tier sibling
**When** failover runs
**Then** the next sibling at the same rank MUST be attempted
