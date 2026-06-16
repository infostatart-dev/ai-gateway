## 1. Provider Config

- [ ] 1.1 Add `github-models` to `ai-gateway/config/embedded/providers.yaml`
- [ ] 1.2 Add curated model capabilities and conservative context windows
- [ ] 1.3 Add `github-models-default` to `credentials.yaml`
- [ ] 1.4 Add provider-limit metadata for free/freemium pacing and cooldowns

## 2. Routing

- [ ] 2.1 Ensure `github-models/<model>` strips only the provider prefix before upstream dispatch
- [ ] 2.2 Ensure required GitHub headers are sent with mock upstream requests
- [ ] 2.3 Decide whether `github-models` joins `autodefault` immediately or only explicit routing

## 3. Docs

- [ ] 3.1 Document `AI_GATEWAY_CREDENTIAL_GITHUB_MODELS_DEFAULT`
- [ ] 3.2 Document GitHub token scope requirements
- [ ] 3.3 Add request examples for direct provider routing and budget-aware routing

## 4. Tests

- [ ] 4.1 Add config parsing tests
- [ ] 4.2 Add credential registry tests
- [ ] 4.3 Add capability filtering tests
- [ ] 4.4 Add mock dispatch tests for path, headers, and model id

## 5. Validation

- [ ] 5.1 Run `mise exec -- openspec validate github-models-provider --strict`
- [ ] 5.2 Run the smallest relevant Rust test slice for provider config/routing
