## ADDED Requirements

### Requirement: GitHub Models provider catalog
The gateway SHALL expose `github-models` as a first-class provider with an OpenAI-compatible chat-completions endpoint, GitHub-specific required headers, and curated model IDs preserved exactly as upstream model names.

#### Scenario: Provider config is loaded
- **WHEN** the embedded provider catalog is loaded
- **THEN** `github-models` is available with base URL `https://models.github.ai/inference/chat/completions`
- **AND** requests include `Authorization: Bearer <credential>`, `X-GitHub-Api-Version: 2022-11-28`, and `Accept: application/vnd.github+json`

### Requirement: GitHub Models credential slot
The gateway SHALL support a `github-models-default` credential slot using `AI_GATEWAY_CREDENTIAL_GITHUB_MODELS_DEFAULT` and SHALL treat additional GitHub Models slots as separate upstream accounts for cooldown and failover.

#### Scenario: Credential is configured
- **WHEN** `AI_GATEWAY_CREDENTIAL_GITHUB_MODELS_DEFAULT` is present
- **THEN** the credential registry includes a `github-models-default` slot for provider `github-models`
- **AND** routing failures for that slot do not cool down unrelated providers or sibling GitHub Models slots

### Requirement: Curated GitHub Models model set
The gateway SHALL include a curated chat model set for GitHub Models covering GPT, o-series, DeepSeek, Llama, Grok, Mistral, Cohere, and Phi model families.

#### Scenario: Model is requested
- **WHEN** a request uses `github-models/openai/gpt-4.1`, `github-models/openai/o3`, or `github-models/deepseek/DeepSeek-R1`
- **THEN** the gateway routes to provider `github-models`
- **AND** the upstream request model is the unprefixed GitHub Models model id

### Requirement: Conservative capability metadata
The gateway SHALL define conservative per-model capability metadata for GitHub Models so tools, strict JSON schema, context windows, and reasoning routing are only enabled where explicitly supported by the catalog.

#### Scenario: Structured routing filters candidates
- **WHEN** a strict JSON schema request is routed through budget-aware selection
- **THEN** GitHub Models candidates without `supports-json-schema: true` are excluded
- **AND** eligible GitHub Models candidates retain their configured context-window metadata

### Requirement: GitHub Models documentation and tests
The gateway SHALL document GitHub Models setup and SHALL test the integration without requiring live GitHub credentials in CI.

#### Scenario: Contributor verifies the integration
- **WHEN** tests run for the GitHub Models provider
- **THEN** config parsing, credential resolution, header construction, model prefix stripping, and mock dispatch are covered
- **AND** docs describe the required GitHub token scope and `AI_GATEWAY_CREDENTIAL_GITHUB_MODELS_DEFAULT`
