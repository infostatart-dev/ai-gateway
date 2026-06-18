## MODIFIED Requirements

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

## ADDED Requirements

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
