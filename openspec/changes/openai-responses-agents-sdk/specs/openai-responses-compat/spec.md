## ADDED Requirements

### Requirement: Declared OpenAI Responses and Agents SDK posture

The project SHALL record posture A, B, C, or D for OpenAI Responses API and Agents SDK compatibility relative to upstream #173 before implying support in docs or routes.

#### Scenario: Operator points Agents SDK at the gateway base URL

- **WHEN** a user configures OpenAI Agents SDK with this gateway as base URL
- **THEN** published docs state whether that configuration is supported, bounded, passthrough, or unsupported
- **AND** silent 404 on `/v1/responses` is not the only discoverability mechanism

### Requirement: Compatibility matrix before non-D implementation

If posture A, B, or C is chosen, the project SHALL publish a compatibility matrix listing minimum API/SDK versions, HTTP paths, streaming behavior, and acceptable failure modes before merge.

#### Scenario: Implementation change starts for posture B

- **WHEN** engineering begins Responses API work
- **THEN** the matrix exists in the change artifacts or linked spec
- **AND** golden-path and negative tests are named in tasks.md before code merge

### Requirement: Chat completions regression guard

Any posture that adds Responses or passthrough routes SHALL preserve existing chat completions behavior unless an explicit breaking release is chosen and semver/changelog rules are followed.

#### Scenario: Responses route ships

- **WHEN** `/v1/responses` or passthrough is added
- **THEN** existing chat completions integration tests continue to pass
- **AND** failures are treated as release-blocking unless documented as breaking
