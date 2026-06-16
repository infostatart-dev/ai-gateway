## 1. Provider Config

- [x] 1.1 Add optional `request-headers` to provider config schema (if missing)
- [x] 1.2 Add `github-models` to `ai-gateway/config/embedded/providers.yaml` with base URL, static headers, and 12 chat models
- [x] 1.3 Add per-model `model-capabilities` and context windows per design.md table
- [x] 1.4 Register embedding model IDs (`text-embedding-3-large`, `text-embedding-3-small`) for catalog only
- [x] 1.5 Add `github-models-default` to `credentials.yaml` (`tier: free`, `budget-rank: 0`)
- [x] 1.6 Add `github-models` free-tier limits to `provider-limits.yaml` (conservative rpm/rpd/concurrent)

## 2. Routing and Dispatch

- [x] 2.1 Ensure `github-models/{publisher}/{model}` preserves publisher prefix upstream
- [x] 2.2 Send `X-GitHub-Api-Version: 2022-11-28` and `Accept: application/vnd.github+json` on dispatch
- [x] 2.3 Insert `github-models` into autodefault after `openrouter`, before `mistral`, gated on credential presence

## 3. Docs

- [x] 3.1 Document `AI_GATEWAY_CREDENTIAL_GITHUB_MODELS_DEFAULT` in `.env.template`
- [x] 3.2 Document `models:read` PAT scope and example requests in `docs/providers.md`
- [x] 3.3 Document autodefault gating and explicit `github-models/<model>` routing

## 4. Tests

- [x] 4.1 Add config parsing tests for provider entry and static headers
- [x] 4.2 Add credential registry tests (present / missing slot)
- [x] 4.3 Add capability filtering tests (o1 reasoning, json-schema exclusion)
- [x] 4.4 Add mock dispatch tests for path, headers, and upstream model id
- [x] 4.5 Add autodefault inclusion test when credential is configured

## 5. Validation

- [x] 5.1 Run `mise exec -- openspec validate github-models-provider --strict`
- [x] 5.2 Run targeted Rust tests for provider config, routing, and mock dispatch

## 6. Version bump (`0.3.0-beta.15`)

- [x] 6.1 Bump root `Cargo.toml` **`0.3.0-beta.14` → `0.3.0-beta.15`**
