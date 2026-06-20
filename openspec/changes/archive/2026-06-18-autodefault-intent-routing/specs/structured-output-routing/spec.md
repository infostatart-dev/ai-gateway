## ADDED Requirements

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
