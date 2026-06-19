# autodefault-routing-priority

## ADDED Requirements

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
