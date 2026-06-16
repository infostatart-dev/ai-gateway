## 1. Release and scaffolding

- [x] 1.1 Bump workspace version to `0.3.0-beta.20` in root `Cargo.toml` and `CHANGELOG.md`
- [x] 1.2 Add credential slot stubs for all eight Tier 1 providers in `credentials.yaml`

## 2. Provider catalogs (providers.yaml)

- [x] 2.1 Add `longcat` provider block (base URL, four Flash models, capabilities)
- [x] 2.2 Add `doubao` provider block (`doubao-pro-32k`)
- [x] 2.3 Add `ollama-cloud` provider block (distinct from local `ollama`)
- [x] 2.4 Add `inclusionai` provider block (`inclusion-model`)
- [x] 2.5 Add `sambanova` provider block (five curated models)
- [x] 2.6 Add `bluesminds` provider block (free-tier subset)
- [x] 2.7 Add `bazaarlink` provider block (`auto:free` + curated models)
- [x] 2.8 Add `cohere` provider block (compatibility `/v1/` base URL, three models)

## 3. Groq free reclassification

- [x] 3.1 Change `groq-default` to `tier: free` / `cost-class: free` in `credentials.yaml`
- [x] 3.2 Add `groq.tiers.free` in `provider-limits.yaml` (mirror developer limits)
- [x] 3.3 Update any tests asserting Groq cost-class or tier

## 4. OpenRouter Tier 2 expansion

- [x] 4.1 Probe `https://openrouter.ai/api/v1/models` and record live `:free` slugs
- [x] 4.2 Add `openrouter/free` and verified `:free` slugs to `providers.yaml`
- [x] 4.3 Add `model-capabilities` for each new OpenRouter free entry
- [x] 4.4 Refresh `openrouter` free-tier notes in `provider-limits.yaml`

## 5. Provider limits (new providers)

- [x] 5.1 Add `longcat` free tier limits and documentation notes
- [x] 5.2 Add `doubao`, `ollama-cloud`, `inclusionai` free tier limits
- [x] 5.3 Add `sambanova`, `bluesminds`, `bazaarlink`, `cohere` free tier limits

## 6. Routing and capabilities

- [x] 6.1 Extend `autodefault_provider_order()` in `read.rs` per spec priority table
- [x] 6.2 Add named capability helpers in `providers.rs` for new providers
- [x] 6.3 Extend `model-mapping.yaml` cost-first entries for `gpt-5.4-nano` and `gpt-5-mini`
- [x] 6.4 Update autodefault scenario tests in `selection.rs` for new order

## 7. Secrets, env, and docs

- [x] 7.1 Add env vars to `.env.template` for all new credential slots
- [x] 7.2 Extend `dev/secrets.local.example.yaml` with commented examples
- [x] 7.3 Document Tier 1 providers in `docs/providers.md`
- [x] 7.4 Document credential env vars in `docs/credentials.md`

## 8. Tests and validation

- [x] 8.1 Add config/credential parsing tests for each new slot
- [x] 8.2 Add autodefault priority tests (LongCat before OpenRouter, Groq free class)
- [x] 8.3 Add capability/json-schema filter tests for representative models
- [x] 8.4 Add mock dispatch tests for at least one model per new provider
- [x] 8.5 Run `cargo clippy` and targeted tests for touched modules
