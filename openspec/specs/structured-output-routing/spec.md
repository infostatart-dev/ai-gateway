# structured-output-routing

## Purpose

Order json_schema-capable autodefault candidates by structured-output reliability
while preserving budget rank as the primary sort key and keeping response-schema
validation unchanged.

## Requirements

### Requirement: json_schema-aware candidate ordering
When a request requires `json_schema` structured output, the router SHALL order
eligible candidates so that providers with proven strict-schema reliability are
preferred, while preserving budget-rank as the primary ordering key. Ordering
SHALL NOT promote a candidate that does not advertise json_schema support.

#### Scenario: Strict-schema request prefers reliable providers within budget order
- **WHEN** a request sets `response_format.type = json_schema`
- **THEN** json_schema-capable providers are preferred over non-capable peers at the same budget rank
- **AND** budget rank remains the primary sort key

#### Scenario: Non-capable provider is not promoted
- **WHEN** a candidate does not advertise json_schema support
- **THEN** structured-output ordering does not move it ahead of capable candidates

### Requirement: Demotion of frequent strict-schema rejectors
The router SHALL allow configuration-driven demotion of providers that are known
or observed to reject the request schema for structured-output requests, so that
repeated wasted strict-schema failovers are reduced without removing the provider
from the candidate set.

#### Scenario: Known rejector is demoted, not removed
- **WHEN** a provider is configured as a frequent strict-schema rejector
- **THEN** it is ranked after more reliable structured-output providers
- **AND** it remains available as a later fallback candidate

### Requirement: Response schema validation is unchanged
The router SHALL continue to validate non-streaming structured-output responses
against the requested json_schema and fail over on invalid output, independent of
the ordering changes introduced by this capability.

#### Scenario: Invalid structured output still fails over
- **WHEN** a json_schema-capable candidate returns content that fails schema validation
- **AND** another candidate remains
- **THEN** the router fails over to the next candidate

### Requirement: Structured output ordering within intent tier bands
The gateway SHALL apply structured-output ordering within each intent tier band
before comparing across tiers when autodefault uses intent mode and the request
requires json_schema. The gateway SHALL NOT promote a deep-tier upstream ahead
of a fast-thinking json_schema-capable upstream when the client intent is
fast-thinking.

#### Scenario: Mini json strict keeps fast-thinking providers first
- **WHEN** a fast-thinking intent request for gpt-5-mini requires json_schema
- **AND** a fast-thinking scout and a deep-tier reasoning model both support json_schema
- **THEN** the fast-thinking candidate is ordered before the deep-tier candidate in the preferred band

#### Scenario: json_schema_rank applies within same intent tier
- **WHEN** two fast-thinking candidates both support json_schema but differ in json_schema_rank
- **THEN** the higher json_schema_rank fast-thinking candidate is preferred after cost-class and budget-rank

#### Scenario: Plain mini does not require json_schema for eligibility
- **WHEN** a fast-thinking plain request for gpt-5-mini omits json_schema
- **THEN** structured-output ordering does not exclude non-json_schema upstream from the eligible pool
