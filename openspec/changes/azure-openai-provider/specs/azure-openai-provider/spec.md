## ADDED Requirements

### Requirement: Explicit Azure OpenAI posture

The project SHALL record adopt, defer, or document-only posture for native Azure OpenAI support before claiming compatibility in operator-facing docs.

#### Scenario: User references issue #289 class confusion

- **WHEN** documentation or marketing implies Azure works in open-source ai-gateway
- **THEN** `openspec/changes/azure-openai-provider/design.md` states adopt, defer, or document-only
- **AND** README or linked doc reflects the same posture without ambiguity

### Requirement: Adopt-tier Azure contract

If **adopt** is chosen, the gateway SHALL document resource host, deployment id, `api-version`, and supported credential modes with unit and mock-integration tests — without requiring a live Azure subscription in CI.

#### Scenario: Adopt decision is recorded

- **WHEN** design.md records **adopt**
- **THEN** a follow-up implementation change defines HTTP path mapping and auth v1 scope explicitly
- **AND** Entra or managed identity beyond v1 scope is listed as out-of-scope or phase 2
