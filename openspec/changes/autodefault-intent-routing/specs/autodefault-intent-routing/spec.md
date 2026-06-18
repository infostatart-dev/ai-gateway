## ADDED Requirements

### Requirement: Routing intent extraction from client model name
The gateway SHALL derive a `RoutingIntent` from the client-requested model name
and request payload before autodefault candidate selection. The intent SHALL
include at minimum: `preferred_tier`, `floor_tier`, and `escalation_ceiling`.

Intent tiers SHALL be: `fast`, `fast-thinking`, `standard`, and `deep`.

Name heuristics SHALL use longest-match-first rules:

- `gpt-5-nano` and `gpt-5-mini` (and versioned aliases such as `gpt-5.4-nano`,
  `gpt-5.4-mini`) SHALL resolve to **fast-thinking** intent — the client wants
  **fast answers that still think**, not dumb flash-only models and not deep
  slow reasoning.
- Plain `gpt-5` without nano/mini suffix SHALL resolve to **deep** intent.
- Models matching `o1`, `o3`, `o4`, `reasoner`, or `thinking` SHALL resolve to
  **deep** intent.

`gpt-5-nano` and `gpt-5-mini` SHALL resolve to the **same** intent tier and
floor; the gateway SHALL NOT treat them as separate binding keys in intent mode.

#### Scenario: Mini resolves fast-thinking intent
- **WHEN** a request uses model `openai/gpt-5-mini`
- **THEN** `preferred_tier` is fast-thinking
- **AND** `floor_tier` is fast-thinking
- **AND** `escalation_ceiling` is deep

#### Scenario: Nano resolves same intent as mini
- **WHEN** a request uses model `openai/gpt-5-nano`
- **THEN** `preferred_tier` is fast-thinking
- **AND** `floor_tier` is fast-thinking
- **AND** the resolved intent equals that of `openai/gpt-5-mini` for the same payload

#### Scenario: Plain gpt-5 resolves deep intent
- **WHEN** a request uses model `openai/gpt-5` without nano or mini suffix
- **THEN** `preferred_tier` is deep
- **AND** `floor_tier` is deep

### Requirement: Payload shape widens or narrows the capable pool
After intent tier is resolved, the gateway SHALL apply payload-derived hard
requirements before ranking. Structured-output shape SHALL NOT change the
client intent tier; it SHALL only change which upstream candidates are eligible.

When `response_format.type` is `json_schema`, the gateway SHALL require upstream
json_schema support and SHALL exclude candidates that do not advertise
json_schema capability.

When the request is plain (no json_schema requirement), the gateway SHALL NOT
exclude upstream candidates solely for lacking json_schema support.

#### Scenario: Mini json strict narrows to json_schema-capable pool
- **WHEN** autodefault receives `openai/gpt-5-mini` with strict json_schema
- **THEN** every selected candidate supports json_schema
- **AND** candidates without json_schema support are not attempted

#### Scenario: Mini plain allows non-json upstream
- **WHEN** autodefault receives `openai/gpt-5-mini` without json_schema
- **THEN** json_schema-capable and non-json_schema-capable upstream candidates
  are both eligible within the fast-thinking intent band
- **AND** ranking proceeds among the combined eligible pool

#### Scenario: Nano json strict narrows to json_schema-capable pool
- **WHEN** autodefault receives `openai/gpt-5-nano` with strict json_schema
- **THEN** every selected candidate supports json_schema
- **AND** candidates without json_schema support are not attempted

#### Scenario: Nano plain allows non-json upstream
- **WHEN** autodefault receives `openai/gpt-5-nano` without json_schema
- **THEN** json_schema-capable and non-json_schema-capable upstream candidates
  are both eligible within the fast-thinking intent band
- **AND** ranking proceeds among the combined eligible pool

### Requirement: Canonical acceptance matrix for gpt-5-mini and gpt-5-nano
The gateway SHALL satisfy the following four canonical autodefault acceptance
cases. In each case, selection SHALL start from the global intent pool (not
model-mapping alias binding), apply the payload filter from the previous
requirement, then rank survivors by cost-class, cooldown availability, and
existing budget-aware provider priority.

#### Scenario: Acceptance A — gpt-5-mini json strict
- **WHEN** autodefault receives `openai/gpt-5-mini` with strict json_schema
- **THEN** client intent is fast-thinking
- **AND** only json_schema-capable upstream candidates in the fast-thinking band
  are attempted before any escalation
- **AND** the first hop is not a deep-tier reasoning model while free
  fast-thinking json_schema-capable candidates remain available

#### Scenario: Acceptance B — gpt-5-mini plain
- **WHEN** autodefault receives `openai/gpt-5-mini` without json_schema
- **THEN** client intent is fast-thinking
- **AND** both json_schema-capable and non-json_schema upstream candidates in
  the fast-thinking band are eligible
- **AND** ranking prefers available free candidates before paid or cooled-down
  paths

#### Scenario: Acceptance C — gpt-5-nano json strict
- **WHEN** autodefault receives `openai/gpt-5-nano` with strict json_schema
- **THEN** client intent is fast-thinking (same as mini)
- **AND** only json_schema-capable upstream candidates in the fast-thinking band
  are attempted before any escalation
- **AND** the first hop is not a deep-tier reasoning model while free
  fast-thinking json_schema-capable candidates remain available

#### Scenario: Acceptance D — gpt-5-nano plain
- **WHEN** autodefault receives `openai/gpt-5-nano` without json_schema
- **THEN** client intent is fast-thinking (same as mini)
- **AND** both json_schema-capable and non-json_schema upstream candidates in
  the fast-thinking band are eligible
- **AND** ranking prefers available free candidates before paid or cooled-down
  paths

### Requirement: Intent pool selection for autodefault
When router `source-model-selection` is `intent`, the gateway SHALL select
candidates from the full credential×model pool using intent tier metadata and
payload hard requirements. The gateway SHALL NOT require the candidate upstream
model to appear in model-mapping.yaml for the client-requested alias.

#### Scenario: Nano request may use llama scout without nano mapping entry
- **WHEN** autodefault receives `openai/gpt-5-nano` with json_schema
- **AND** groq llama-4-scout is configured and json_schema-capable
- **AND** scout is not listed under gpt-5-nano in model-mapping.yaml
- **THEN** scout remains an eligible candidate in intent mode

#### Scenario: Strict mode preserves binding gate
- **WHEN** a named router has `source-model-selection: strict`
- **THEN** candidate selection uses matches_source_model against model-mapping.yaml
- **AND** behavior matches pre-change autodefault binding semantics

#### Scenario: Autodefault defaults to intent mode
- **WHEN** sidecar mode builds router `autodefault`
- **THEN** `source-model-selection` is `intent`

### Requirement: Upstream intent tier metadata
Each upstream model in the provider catalog SHALL carry an intent_tier used for
filtering and ranking in intent mode. The gateway SHALL tag json_schema-capable
scout, gpt-oss, and similar economy reasoning models as fast-thinking. The
gateway SHALL tag dumb flash or instant slugs as fast. The gateway SHALL tag
reasoning and large flagship slugs as deep.

#### Scenario: Scout model tagged fast-thinking
- **WHEN** catalog includes groq llama-4-scout
- **THEN** its resolved intent_tier is fast-thinking

#### Scenario: Reasoning model tagged deep
- **WHEN** catalog includes a model whose slug matches deep-tier heuristics
- **THEN** its resolved intent_tier is deep

### Requirement: Preferred-tier-first ranking
In intent mode, the gateway SHALL rank and attempt candidates at preferred_tier
before any higher tier. Within the same tier, existing cost-class-first
budget-aware ordering and cooldown availability SHALL apply unchanged.

#### Scenario: Mini json strict tries fast-thinking before deep
- **WHEN** autodefault receives a fast-thinking intent request with json_schema
- **AND** both fast-thinking and deep json_schema-capable candidates exist
- **THEN** all fast-thinking candidates are ordered and attempted before any
  deep-tier candidate

#### Scenario: Cost-class still primary within tier
- **WHEN** two fast-thinking candidates differ by cost-class
- **THEN** the free cost-class candidate is ranked before the paid candidate

#### Scenario: Availability breaks ties within tier
- **WHEN** two fast-thinking candidates are in the same cost-class band
- **AND** one candidate is in cooldown and the other is available
- **THEN** the available candidate is ranked before the cooled-down candidate

### Requirement: Asymmetric stability escalation
The gateway SHALL escalate to higher intent tiers up to the request escalation
ceiling when every preferred-tier candidate is exhausted by failover, and SHALL
NOT select any candidate below the request floor tier.

#### Scenario: Nano escalates to larger model for stability
- **WHEN** a fast-thinking request exhausts all fast-thinking candidates
- **AND** standard or deep candidates remain capable
- **THEN** the gateway attempts standard-tier candidates before deep-tier
- **AND** returns a successful response from an escalated candidate if available

#### Scenario: Deep request never downgrades to scout
- **WHEN** a deep-tier request has floor deep
- **AND** only fast-thinking or fast-tier free candidates are available
- **THEN** those below-floor candidates are not selected
- **AND** the gateway proceeds to deep-tier paths only

#### Scenario: No downgrade below floor on payload best-effort
- **WHEN** payload-aware filtering would use best-effort tail
- **AND** no candidate at or above floor tier fits the payload
- **THEN** the gateway does not select a below-floor candidate
- **AND** returns provider-not-found or honest at-or-above-floor attempt per payload spec

### Requirement: Reasoning misclassification fix
The gateway SHALL NOT classify gpt-5-nano or gpt-5-mini as deep-tier reasoning
solely because the model name contains the substring gpt-5. The gateway SHALL
NOT promote deep-tier upstream models ahead of fast-thinking peers on the first
hop for mini or nano requests.

#### Scenario: Nano json strict does not start on deep reasoning model
- **WHEN** a request uses openai/gpt-5-nano with strict json_schema
- **AND** free fast-thinking json_schema-capable candidates exist
- **THEN** the first attempted candidate is not a deep-tier reasoning model

#### Scenario: Plain gpt-5 prefers reasoning-capable upstream
- **WHEN** a request uses deep-tier intent
- **THEN** reasoning-capable upstream models rank ahead of non-reasoning peers
  at the same cost-class and intent tier

### Requirement: Intent routing observability
The gateway SHALL expose resolved routing intent tier, payload filter mode
(plain vs json_schema), and escalation phase in route trace and response headers
for autodefault intent-mode requests.

#### Scenario: Escalated response is labeled
- **WHEN** a fast-thinking request succeeds on a deep-tier upstream after escalation
- **THEN** response headers indicate escalated selection phase
- **AND** include the resolved upstream model identity

### Requirement: Tests and documentation
The gateway SHALL document intent-based autodefault routing in docs/routing.md
including the four-case acceptance matrix (mini/nano × plain/json strict).
CI SHALL cover all four acceptance scenarios, deep no-downgrade, stability
escalation, and strict-mode regression without live credentials.

#### Scenario: CI covers four-case acceptance matrix
- **WHEN** intent routing tests run
- **THEN** Acceptance A through D pass
- **AND** gpt-5 no-scout and escalation-after-exhaustion cases pass
