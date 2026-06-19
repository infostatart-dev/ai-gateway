# autodefault-routing-priority

## Purpose

Cost-class-first autodefault routing: free API keys before paid API and browser
sessions, with ChatGPT Web as last-resort fallback. Aligns credential metadata,
provider priority, and default nano model bindings with cascade-by-cost policy.
## Requirements
### Requirement: Cost-class metadata on credential slots
The gateway SHALL support a `cost-class` field on embedded credential slots with values `free`, `paid`, and `paid-browser`. When `cost-class` is omitted, the gateway SHALL derive it from `tier` and provider kind.

#### Scenario: Free API slot resolves cost-class
- **WHEN** credential slot `openrouter-default` has `tier: free` and no explicit `cost-class`
- **THEN** the resolved cost-class is `free`

#### Scenario: Paid API slot resolves cost-class
- **WHEN** credential slot `gemini-default` has `tier: tier-3`
- **THEN** the resolved cost-class is `paid`

#### Scenario: ChatGPT Web session resolves paid-browser
- **WHEN** credential slot `chatgpt-web-default` is registered from a session file
- **THEN** the resolved cost-class is `paid-browser`

#### Scenario: DeepSeek Web session resolves free
- **WHEN** credential slot `deepseek-web-default` is registered from a session file
- **THEN** the resolved cost-class is `free`
- **AND** it is ordered after free API keys and Gemini free slots via provider priority

### Requirement: Cost-class-first budget-aware ranking
The gateway SHALL sort autodefault budget-aware candidates by cost-class before
`budget-rank` and provider priority. Ordering SHALL be `free` → `paid` →
`paid-browser` within the active `free-up` tier cascade. In intent selection
mode, cost-class ordering SHALL apply within each intent tier band (preferred
before escalated).

#### Scenario: Free API candidate precedes paid browser
- **WHEN** both `openrouter-default` and `chatgpt-web-default` are available for the same mapped model
- **THEN** the openrouter candidate is ranked before the chatgpt-web candidate

#### Scenario: Paid API precedes paid browser
- **WHEN** both `anthropic-default` and `chatgpt-web-default` are available
- **THEN** the anthropic candidate is ranked before the chatgpt-web candidate

#### Scenario: Budget-rank breaks ties within cost-class
- **WHEN** two `free` credentials differ only by `budget-rank`
- **THEN** the lower `budget-rank` value is tried first

#### Scenario: Intent preferred band before escalation
- **WHEN** autodefault uses intent selection mode
- **AND** fast-tier and deep-tier free candidates both exist for a fast-tier request
- **THEN** cost-class ranking among fast-tier candidates completes before any deep-tier candidate is attempted

### Requirement: Autodefault provider priority order
The gateway SHALL build autodefault with the following provider priority when credentials or session files are available (earlier = higher priority within the same cost-class band):

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

#### Scenario: ChatGPT Web is last resort
- **WHEN** `chatgpt-web` and at least one free API provider are configured
- **THEN** `chatgpt-web` has the lowest autodefault provider priority among configured providers

#### Scenario: DeepSeek Web follows Gemini free
- **WHEN** `gemini-free` and `deepseek-web` are both configured
- **THEN** gemini free slots are ranked before `deepseek-web`

#### Scenario: DeepSeek Web precedes paid Gemini default
- **WHEN** `deepseek-web` and `gemini-default` are both configured
- **AND** no free Gemini slot is available
- **THEN** `deepseek-web` is ranked before the paid `gemini-default` credential

#### Scenario: GitHub Models and OpenCode are free cost-class
- **WHEN** `github-models-default` or `opencode-default` resolve
- **THEN** each slot has cost-class `free`
- **AND** neither uses a separate `subsidized` band in v1

### Requirement: Cost-aligned model binding for default nano model
For routers with `source-model-selection: strict`, the gateway SHALL order
`model-mapping.yaml` entries for `gpt-5.4-nano` so free-tier targets precede
paid API targets. The first entries SHALL mirror the cost-first pattern used for
`gpt-5-mini`: OpenRouter `:free` and OpenCode free models before `anthropic` or
paid `gemini` entries.

For router `autodefault` with `source-model-selection: intent`, per-alias
mapping order SHALL NOT gate candidate selection. Mapping entries remain
documentation and strict-router configuration only.

#### Scenario: Nano mapping prefers free OpenRouter first (strict mode)
- **WHEN** routing `openai/gpt-5.4-nano` through a strict-binding router
- **AND** `openrouter-default` is configured
- **THEN** the first eligible mapping target is `openrouter/openai/gpt-oss-120b:free` or another free-tier openrouter entry before any `anthropic` mapping

#### Scenario: Mapping skips unavailable providers (strict mode)
- **WHEN** the first mapping target's provider has no resolved credential
- **THEN** the gateway tries the next mapping entry in YAML order

#### Scenario: Autodefault intent mode ignores nano mapping gate
- **WHEN** autodefault receives `openai/gpt-5.4-nano` with json_schema
- **AND** a capable free upstream exists that is not listed under `gpt-5.4-nano` in model-mapping.yaml
- **THEN** that upstream is eligible without adding a mapping entry

### Requirement: Autodefault source model selection mode
Sidecar autodefault SHALL be built with `source-model-selection: intent`.
Operator-configured named routers SHALL default to `source-model-selection:
strict` unless explicitly overridden.

#### Scenario: Named router stays strict by default
- **WHEN** an operator defines a custom router in configuration without `source-model-selection`
- **THEN** the router uses strict model binding

#### Scenario: Autodefault uses intent pool
- **WHEN** sidecar mode creates router `autodefault`
- **THEN** `source-model-selection` is `intent`

### Requirement: Canonical autodefault example model
The gateway SHALL use `openai/gpt-5.4-nano` as the documented and CLI-banner default model for `/router/autodefault/chat/completions`. Operators MAY override via `AI_GATEWAY_AUTODEFAULT_DEFAULT_MODEL`.

#### Scenario: Startup banner shows nano default
- **WHEN** the gateway prints the autodefault curl example
- **THEN** the JSON body uses `"model": "openai/gpt-5.4-nano"`

### Requirement: Policy metadata in catalog not env
Cost-class and budget-rank SHALL be defined in embedded `credentials.yaml`. Secret values SHALL remain env-based (`AI_GATEWAY_CREDENTIAL_<ID>`). Cost-class SHALL NOT require a dedicated env var.

#### Scenario: Operator configures only secrets via env
- **WHEN** an operator sets `AI_GATEWAY_CREDENTIAL_OPENROUTER_DEFAULT` in Kubernetes
- **THEN** routing policy for that slot comes from embedded catalog metadata without additional policy env vars

### Requirement: Coordination with payload-aware routing (beta.16)
The gateway SHALL apply cost-class ranking in `effective_budget_rank` before payload-aware filtering reorders survivors. Cost-class SHALL take precedence over `json_schema_rank` and capability-fit tiebreakers.

#### Scenario: Cost-class beats json_schema_rank
- **WHEN** a request requires `json_schema`
- **AND** a `free` openrouter candidate and a `paid` candidate both support schema
- **THEN** the `free` candidate is ranked before the `paid` candidate regardless of `json_schema_rank`

#### Scenario: Tools request skips deepseek-web
- **WHEN** a request includes `tools`
- **AND** `deepseek-web-default` is configured
- **THEN** deepseek-web is not selected (catalog `supports_tools: false`)
- **AND** routing proceeds to other free API or paid paths before `chatgpt-web`

### Requirement: Documentation, tests, release version, and changelog
The gateway SHALL document autodefault priority policy (cost-class bands, provider order, nano default model, ChatGPT Web as last resort, interaction with payload-aware filtering), SHALL test provider order, cost-class ranking, and `gpt-5.4-nano` mapping order without live credentials in CI, and SHALL ship this capability in release **`0.3.0-beta.17`**.

When releasing beta.17, the gateway SHALL backfill `CHANGELOG.md` entries for **`0.3.0-beta.12` through `0.3.0-beta.17`** (gemini multi-account, chatgpt-web stabilization, deepseek-web, github-models, payload-aware routing, and this change). The changelog currently ends at beta.11 while code is at beta.16.

#### Scenario: Contributor verifies routing priority
- **WHEN** tests run for autodefault routing priority
- **THEN** provider order, cost-class sort, chatgpt-web last-resort behavior, deepseek-web placement, `gpt-5.4-nano` mapping order, and beta.16 interaction cases (json_schema + tools) are covered

#### Scenario: Release notes cover beta.12–17
- **WHEN** beta.17 is released
- **THEN** `CHANGELOG.md` includes sections for beta.12 through beta.17
- **AND** beta.17 notes the ChatGPT Web last-resort breaking change for operators who relied on browser-first autodefault

### Requirement: Health-aware autodefault candidate filtering

Autodefault budget-aware selection SHALL apply credential health and dead-provider
filters before cost-class ranking. Circuit-open credentials and pod-lifetime
zero-success providers SHALL NOT appear in the route plan or failover walk.

Cost-class ordering (`free` → `paid` → `paid-browser`) SHALL apply among health
survivors only.

#### Scenario: Dead provider skipped before Gemini

- **WHEN** `cloudflare-default` has zero successes and ≥10 attempts since process start
- **AND** `gemini-free-9` is healthy
- **THEN** the first upstream attempt is not cloudflare
- **AND** the first attempt targets a free-tier survivor (gemini or openrouter)

#### Scenario: Cost-class preserved among healthy candidates

- **WHEN** healthy `openrouter-default` and healthy `chatgpt-web-default` both exist
- **THEN** openrouter appears in the plan before chatgpt-web

### Requirement: Planned chain replaces full-pool walk

Autodefault SHALL use `route-chain-planning` output as the ordered candidate list
for failover. Full `candidates` vec SHALL NOT be walked directly except during the
single replan fallback described in `route-chain-planning`.

#### Scenario: Autodefault respects plan order

- **WHEN** autodefault builds a plan with first hop `gemini-free-10`
- **THEN** the failover loop's first upstream attempt uses `gemini-free-10`

