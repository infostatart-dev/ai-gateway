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
