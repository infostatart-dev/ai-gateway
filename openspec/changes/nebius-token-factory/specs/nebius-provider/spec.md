## ADDED Requirements

### Requirement: Explicit Nebius integration decision

The project SHALL record an adopt, defer, or partial decision for Nebius Token Factory parity with upstream PR #299 before implementation begins.

#### Scenario: Operator asks for Nebius support

- **WHEN** a contributor or operator requests Nebius routes or credentials
- **THEN** `openspec/changes/nebius-token-factory/design.md` states the current decision tier
- **AND** unsupported tiers are not implied by README or config examples

### Requirement: Adopt-tier parity bar

If the decision is **adopt**, the gateway SHALL expose documented `/nebius/v1/*` OpenAI-compatible routes, env-based credentials, routing tests, and mapping policy consistent with other first-class providers.

#### Scenario: Adopt decision is recorded

- **WHEN** design.md records **adopt**
- **THEN** a follow-up implementation change MUST include unit and integration tests with mock upstream
- **AND** `NEBIUS_API_KEY` (or fork-canonical name) appears in the same documentation surfaces as peer providers
