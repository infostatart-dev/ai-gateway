## Why

GitHub Models is a strong low-friction provider candidate: it exposes useful free model access through a normal GitHub token and uses an OpenAI-compatible chat-completions shape. Adding it gives the gateway another high-quality free/freemium fallback without browser sessions or custom auth flows.

## What Changes

- Add GitHub Models as a first-class provider.
- Support GitHub PAT credentials through the existing credential-slot model.
- Register curated model IDs for chat completions, including GPT, o-series, DeepSeek, Llama, Grok, Mistral, Cohere, and Phi entries.
- Add provider limits and documentation for the free/freemium operating profile.
- Add mock-backed tests for config loading, credential resolution, routing, headers, and dispatch.

## Capabilities

### New Capabilities

- `github-models-provider`: GitHub Models provider integration, credentials, model catalog, routing, and tests.

### Modified Capabilities

- None.

## Impact

- Provider catalog: `ai-gateway/config/embedded/providers.yaml`.
- Credential slots: `ai-gateway/config/embedded/credentials.yaml`, `.env.template`, and docs.
- Provider limits and cooldowns: `ai-gateway/config/embedded/provider-limits.yaml`.
- Routing/model metadata: capability routing, model mapping, and mock dispatch tests.
