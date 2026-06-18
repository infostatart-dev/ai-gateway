# autodefault-intent-routing

## ADDED Requirements

### Requirement: Client-ordered stability escalation inside route plan

The gateway SHALL, when autodefault uses intent selection mode and
`route-chain-planning` builds an intra-slot ladder for Gemini free credentials,
require the planner to append capacity and stability ladder bands on the same
credential **upward** before escalating to a higher intent tier on another provider.

This escalation serves **client-ordered routing stability**: deliver a reliable
answer by moving to larger/stable models on the same project (e.g.
`gemini-2.5-flash-lite`) rather than jumping to an unrelated faster/cheaper model
on another provider.

The gateway SHALL NOT downgrade below `floor_tier` for the routing intent derived
from the request model when selecting cross-provider hops.

The gateway SHALL NOT select openrouter deprioritized models while any healthy
Gemini slot has stability-band headroom per `quota-headroom-scheduling`.

#### Scenario: Json strict fast-thinking escalates to flash-lite before openrouter

- **WHEN** autodefault receives `openai/gpt-5-mini` with strict json_schema
- **AND** fast-band Gemini models on the preferred slot have zero quota headroom
- **AND** `gemini-3.1-flash-lite` on that slot supports json_schema and has headroom
- **THEN** the route plan includes the flash-lite hop before any openrouter hop

#### Scenario: Stability band preferred over nemotron

- **WHEN** fast and capacity bands on all healthy Gemini slots are exhausted
- **AND** `gemini-2.5-flash-lite` on `gemini-free-10` has headroom
- **AND** openrouter nemotron is configured in deprioritized band
- **THEN** the plan includes `gemini-2.5-flash-lite` before nemotron

#### Scenario: Escalation ceiling still applies cross-provider

- **WHEN** all ladder models on all healthy Gemini slots are exhausted
- **AND** openrouter and deepseek-web are healthy
- **THEN** the plan may include those providers within `escalation_ceiling`
- **AND** no provider below `floor_tier` is selected

#### Scenario: Never downgrade on replan

- **WHEN** a stability-band hop fails with failoverable error
- **THEN** replan does not insert a faster model below the failed hop's ladder band
