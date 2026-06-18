## ADDED Requirements

### Requirement: Intra-slot stability complements global intent escalation

The gateway SHALL run intra-slot **stability band** escalation within the same
credential before global intent escalation crosses providers or paid fallback,
when the provider has a model ladder and `quota-profile: per-model`. Stability
band selection SHALL satisfy the client intent floor and SHALL prefer larger or
higher-capacity free models on the same slot. The gateway SHALL NOT select a
stability-band model that is smaller or less capable than models already
attempted in the fast band on that slot.

#### Scenario: Gemini stability uses larger free flash-lite not smaller tier

- **WHEN** a fast-thinking request exhausts `gemini-3-flash-preview` and `gemini-3.5-flash` on `gemini-free-8`
- **AND** stability band includes `gemini-2.5-flash-lite`
- **THEN** the gateway attempts `gemini-2.5-flash-lite` on `gemini-free-8`
- **AND** does not attempt a smaller or legacy flash slug below the fast band capability

#### Scenario: Stability does not downgrade below intent floor across providers

- **WHEN** a fast-thinking request has `floor_tier: fast-thinking`
- **AND** intra-slot stability is exhausted on all free Gemini slots
- **THEN** global escalation may proceed to standard/deep tiers on other providers
- **AND** the gateway does not select fast-only upstream below the floor

#### Scenario: Client stability before cross-provider escalation

- **WHEN** fast-thinking candidates on other providers are available
- **AND** `gemini-3.1-flash-lite` remains available on the current Gemini slot ladder
- **THEN** the gateway completes the intra-slot ladder on the current credential before switching provider for cost

---

## MODIFIED Requirements

### Requirement: Asymmetric stability escalation

The gateway SHALL escalate to higher intent tiers up to the request escalation
ceiling when every preferred-tier candidate is exhausted by failover, and SHALL
NOT select any candidate below the request floor tier.

On providers with per-model ladders, **intra-slot stability band** attempts SHALL
count as upward escalation within the same provider and credential before
cross-provider intent escalation is considered.

#### Scenario: Nano escalates to larger model for stability

- **WHEN** a fast-thinking request exhausts all fast-thinking candidates
- **AND** standard or deep candidates remain capable
- **THEN** the gateway attempts standard-tier candidates before deep-tier
- **AND** returns a successful response from an escalated candidate if available

#### Scenario: Gemini slot stability before groq escalation

- **WHEN** a fast-thinking json_schema request exhausts fast band on `gemini-free-8`
- **AND** `gemini-3.1-flash-lite` on the same slot supports json_schema and has quota
- **THEN** the gateway attempts `gemini-3.1-flash-lite` before selecting a groq fast-thinking candidate

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
