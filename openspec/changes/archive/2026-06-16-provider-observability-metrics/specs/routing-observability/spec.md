## ADDED Requirements

### Requirement: Terminal routing summary includes generation efficiency

At the end of a router request, the structured routing trace summary SHALL include
`generation_ms_per_output_token` (nullable), `upstream_attempts`, and `terminal_outcome`
when an upstream attempt was made.

#### Scenario: Summary after failover success

- **WHEN** a request fails once then succeeds with terminal `output_tokens=20`, duration
  `800 ms`, and `tfft_ms=200`
- **THEN** the summary reports `upstream_attempts=2`
- **AND** `terminal_outcome=success`
- **AND** `generation_ms_per_output_token=30.0`

#### Scenario: Summary on terminal failure

- **WHEN** all upstream attempts fail without a successful body
- **THEN** the summary reports `upstream_attempts` equal to the number of attempts
- **AND** `terminal_outcome` reflects the last failure class
- **AND** `generation_ms_per_output_token` is null
