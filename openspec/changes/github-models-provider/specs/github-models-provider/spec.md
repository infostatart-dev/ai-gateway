## ADDED Requirements

### Requirement: GitHub Models provider catalog
The gateway SHALL expose `github-models` as a first-class OpenAI-compatible provider for GitHub Models inference. The provider SHALL use base URL `https://models.github.ai/inference/chat/completions`, SHALL preserve upstream model IDs exactly as `{publisher}/{model_name}`, and SHALL send GitHub-required static request headers on every upstream call.

#### Scenario: Provider config is loaded
- **WHEN** the embedded provider catalog is loaded
- **THEN** `github-models` is available with base URL `https://models.github.ai/inference/chat/completions`
- **AND** each upstream request includes `Authorization: Bearer <credential>`, `X-GitHub-Api-Version: 2022-11-28`, and `Accept: application/vnd.github+json`

#### Scenario: Provider id is distinct from Copilot
- **WHEN** operators configure GitHub integrations
- **THEN** provider id `github-models` is used for GitHub Models PAT inference
- **AND** it is not overloaded with OAuth-based GitHub Copilot routing

### Requirement: GitHub Models static request headers
The gateway SHALL attach provider-specific static headers from embedded config for `github-models` on the OpenAI-compatible dispatcher path, without requiring operators to set them per request.

#### Scenario: Mock upstream receives GitHub headers
- **WHEN** a chat completion is dispatched to `github-models`
- **THEN** the upstream HTTP request includes `X-GitHub-Api-Version: 2022-11-28`
- **AND** `Accept: application/vnd.github+json`

### Requirement: GitHub Models credential slot
The gateway SHALL support a `github-models-default` credential slot using `AI_GATEWAY_CREDENTIAL_GITHUB_MODELS_DEFAULT`. The slot SHALL use `tier: free` and `budget-rank` aligned with other free API-key providers. Additional GitHub Models slots SHALL be treated as separate upstream accounts for cooldown and failover.

#### Scenario: Credential is configured
- **WHEN** `AI_GATEWAY_CREDENTIAL_GITHUB_MODELS_DEFAULT` is present
- **THEN** the credential registry includes a `github-models-default` slot for provider `github-models`
- **AND** routing failures for that slot do not cool down unrelated providers or sibling GitHub Models slots

#### Scenario: Missing credential is skipped
- **WHEN** `AI_GATEWAY_CREDENTIAL_GITHUB_MODELS_DEFAULT` is unset or empty
- **THEN** slot `github-models-default` is omitted from the credential registry
- **AND** startup completes without error

#### Scenario: Token scope is documented
- **WHEN** a contributor reads GitHub Models setup docs
- **THEN** docs state that the PAT must include `models:read` (fine-grained PAT or equivalent GitHub Models access)

### Requirement: Curated GitHub Models chat catalog
The gateway SHALL register the following chat models for `github-models`, preserving upstream IDs verbatim:

| Model ID | Default context window |
| --- | ---: |
| `openai/gpt-4.1` | 1047576 |
| `openai/gpt-4o` | 128000 |
| `openai/gpt-4o-mini` | 128000 |
| `openai/o1` | 200000 |
| `openai/o3` | 200000 |
| `openai/o4-mini` | 200000 |
| `deepseek/DeepSeek-R1` | 131072 |
| `meta/Llama-4-Maverick-17B-128E-Instruct` | 131072 |
| `xai/grok-3` | 131072 |
| `mistral-ai/Mistral-Medium-3` | 128000 |
| `cohere/Cohere-command-a` | 128000 |
| `microsoft/Phi-4` | 16384 |

The gateway SHALL also list embedding model IDs `openai/text-embedding-3-large` and `openai/text-embedding-3-small` in the catalog for discovery, but SHALL NOT route embeddings traffic through this change.

#### Scenario: Chat model is requested with gateway prefix
- **WHEN** a request uses `github-models/openai/gpt-4.1`, `github-models/openai/o3`, or `github-models/deepseek/DeepSeek-R1`
- **THEN** the gateway routes to provider `github-models`
- **AND** the upstream request body uses model `openai/gpt-4.1`, `openai/o3`, or `deepseek/DeepSeek-R1` respectively

#### Scenario: Publisher prefix is preserved
- **WHEN** a request uses model id `github-models/openai/gpt-4o-mini`
- **THEN** only the `github-models/` gateway prefix is removed
- **AND** upstream model id remains `openai/gpt-4o-mini`

### Requirement: Conservative capability metadata
The gateway SHALL define conservative per-model capability metadata for GitHub Models chat models so tools, strict JSON schema, context windows, and reasoning routing are only enabled where explicitly supported.

#### Scenario: Structured routing filters candidates
- **WHEN** a strict JSON schema request is routed through budget-aware selection
- **THEN** GitHub Models candidates without `supports-json-schema: true` are excluded
- **AND** eligible GitHub Models candidates retain their configured context-window metadata

#### Scenario: Reasoning models are flagged
- **WHEN** routing considers `github-models/openai/o1`, `github-models/openai/o3`, `github-models/openai/o4-mini`, or `github-models/deepseek/DeepSeek-R1`
- **THEN** those catalog entries include `reasoning: true`
- **AND** `openai/o1` does not advertise tools or strict JSON schema support

### Requirement: Autodefault inclusion when configured
The gateway SHALL include `github-models` in the built-in `autodefault` router only when `github-models-default` resolves at startup. When included, it SHALL rank after `openrouter` and before `mistral` in provider priority order.

#### Scenario: Autodefault with GitHub Models credential
- **WHEN** `AI_GATEWAY_CREDENTIAL_GITHUB_MODELS_DEFAULT` is set
- **AND** the autodefault router is built at startup
- **THEN** `github-models` is an eligible autodefault provider
- **AND** it is ordered after `openrouter` and before `mistral`

#### Scenario: Autodefault without GitHub Models credential
- **WHEN** `AI_GATEWAY_CREDENTIAL_GITHUB_MODELS_DEFAULT` is not set
- **THEN** `github-models` is omitted from autodefault
- **AND** explicit `github-models/<model>` routing remains unavailable without a configured slot

### Requirement: Free-tier provider limits
The gateway SHALL document and apply conservative free-tier limits for `github-models` in `provider-limits.yaml`, reflecting GitHub Models per-model rate tiers (requests per minute/day and concurrent requests vary by model; high-tier chat models are typically ~10 RPM and ~50 RPD).

#### Scenario: Provider limits are loaded
- **WHEN** embedded provider limits are loaded
- **THEN** `github-models` defines a `free` tier with conservative chat-completions limits
- **AND** notes reference GitHub Models rate-limit tiers and per-account scope

### Requirement: GitHub Models documentation, tests, and release version
The gateway SHALL document GitHub Models setup (PAT scope, env var, model prefix examples, autodefault behavior), SHALL test the integration without live GitHub credentials in CI, and SHALL ship this capability in release **`0.3.0-beta.15`**.

#### Scenario: Contributor verifies the integration
- **WHEN** tests run for the GitHub Models provider
- **THEN** config parsing, credential resolution, static header construction, gateway-prefix stripping, capability filtering, autodefault gating, and mock dispatch are covered
- **AND** docs describe `AI_GATEWAY_CREDENTIAL_GITHUB_MODELS_DEFAULT` and required `models:read` scope
