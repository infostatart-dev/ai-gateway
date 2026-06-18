## ADDED Requirements

### Requirement: Slot restriction failover in autodefault walk

When a candidate fails with credential restriction, autodefault SHALL treat the
failure as **slot-scoped** and continue the ordered candidate walk. The router
SHALL prefer delivering a successful response via another credential, provider, or
**stability-band escalation up** on a different slot over returning `403` to the
client when an eligible candidate remains.

Slot restriction SHALL NOT trigger model downgrade on the same credential.

#### Scenario: Restricted DeepSeek slot fails over to second session

- **WHEN** autodefault selects `deepseek-web-default`
- **AND** dispatch returns credential restriction
- **AND** `deepseek-web-2` is registered
- **THEN** autodefault attempts `deepseek-web-2` before returning failure to the client

#### Scenario: Inter-provider stability band after all DeepSeek slots restricted

- **WHEN** every DeepSeek Web credential slot in the walk is restricted or skipped
- **AND** a Gemini free credential has a stability-band model that satisfies the routing intent
- **THEN** autodefault MAY select that stability-band candidate
- **AND** stability escalation remains **up** within the target slot (no downgrade below fast band capability)

#### Scenario: Intent floor preserved across restriction failover

- **WHEN** a fast-thinking json_schema request hits credential restriction on DeepSeek
- **AND** failover reaches a Gemini stability-band candidate that supports json_schema
- **THEN** autodefault completes without promoting a deep-reasoning model ahead of the intent tier band

#### Scenario: Four-slot pool isolates mute per credential

- **WHEN** autodefault walks four DeepSeek Web credential slots (`deepseek-web-default`
  through `deepseek-web-4`)
- **AND** slot `deepseek-web-default` returns credential restriction
- **THEN** slots `deepseek-web-2`, `deepseek-web-3`, and `deepseek-web-4` remain
  eligible in the same request walk
- **AND** mute on slot 1 does not poison slots 2–4

#### Scenario: Partial mute prefix skips only muted slots

- **WHEN** the first **K** of four DeepSeek slots (K ∈ {1, 2, 3}) return credential
  restriction in order
- **AND** slot **K+1** is healthy
- **THEN** autodefault succeeds via slot **K+1**
- **AND** does not require attempting healthy slots beyond the first success unless
  structured-output validation fails
