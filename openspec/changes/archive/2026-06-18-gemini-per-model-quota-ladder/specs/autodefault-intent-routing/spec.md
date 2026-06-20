## ADDED Requirements

### Requirement: Gemini slot stability complements intent escalation

The gateway SHALL apply Gemini free model ladder selection within each credential
when autodefault uses intent mode. The gateway SHALL use stability-band models on
the same credential only after fast and capacity bands are exhausted. The gateway
SHALL preserve the client intent floor and SHALL defer cross-provider deep
escalation until intra-slot and inter-slot Gemini options are exhausted.

#### Scenario: Fast-thinking request uses fast ladder before stability pro

- **WHEN** autodefault receives `openai/gpt-5-mini` with fast-thinking intent
- **AND** `gemini-3-flash-preview` on `gemini-free-8` is gated out
- **AND** `gemini-3.1-flash-lite` on the same slot is eligible
- **THEN** the gateway selects `gemini-3.1-flash-lite` before `gemini-2.5-pro`

#### Scenario: Stability pro before cross-provider deep escalation

- **WHEN** all fast-thinking Gemini models are exhausted on all configured free slots for a request
- **AND** `gemini-2.5-pro` on an eligible free slot supports request capabilities
- **THEN** the gateway SHALL attempt stability-band Gemini on that slot before selecting a deep-tier provider outside Gemini

#### Scenario: No downgrade below intent floor via Gemini ladder

- **WHEN** client intent floor is fast-thinking with strict json_schema
- **THEN** the gateway SHALL NOT select Gemini models that fail capability or intent floor checks solely because they are in the capacity band
