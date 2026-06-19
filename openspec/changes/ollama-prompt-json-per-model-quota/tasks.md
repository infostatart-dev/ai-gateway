## 1. Catalog and capability metadata

- [ ] 1.1 Add `json-schema-delivery` enum to provider model capability YAML schema and types
- [ ] 1.2 Set `ollama-cloud/gpt-oss:120b` → `prompt`, `intent-tier: fast-thinking`
- [ ] 1.3 Set `ollama-cloud/gpt-oss:20b` → `prompt`, `intent-tier: fast`
- [ ] 1.4 Map delivery enum to runtime `supports_json_schema` + default `json_schema_rank` in `capability/providers.rs`
- [ ] 1.5 Update `provider-limits.yaml` ollama-cloud: `quota-profile: per-model`, weighted-quota notes, verification date
- [ ] 1.6 Trim ollama-cloud catalog to `gpt-oss:120b` + `gpt-oss:20b`; remove `glm-4.7` and other unverified slugs
- [ ] 1.7 Document removed slugs in English in `provider-limits.yaml` notes (identity/behavior unclear)
- [ ] 1.8 Set ollama-cloud free ladder to gpt-oss slugs only (120b thinking, 20b fast)

## 2. Prompt-json upstream primitives

- [ ] 2.1 Generalize `chatgpt_json_schema` injection for reuse (rename module or shared `prompt_json_schema`)
- [ ] 2.2 Hook prompt-json inject + strip `response_format` in ollama-cloud OpenAI-compatible converter
- [ ] 2.3 Implement reflection turn builder (invalid content + schema corrective user message)
- [ ] 2.4 Wire reflection executor: exactly 1 retry on validation failure (no second reflection)
- [ ] 2.5 Add dedicated JSON-validation cooldown registry (24h, Model scope; separate from 404 exhaustion store)

## 3. Router integration

- [ ] 3.1 Extend structured-output sort: native delivery before prompt at same budget rank
- [ ] 3.2 Classify Ollama 403 subscription → `ExhaustionScope::Model` in `quota_scope.rs`
- [ ] 3.3 Skip JSON-validation-cooled models in ladder filter (read dedicated registry, not 404 store)
- [ ] 3.4 Emit trace + metrics for `json_schema_prompt_exhausted` and reflection success

## 4. Routing-load and integration tests

- [ ] 4.1 Scenario: ollama prompt-json call 1 valid (positive)
- [ ] 4.2 Scenario: ollama prose → reflection → valid JSON (positive recovery)
- [ ] 4.3 Scenario: ollama double failure → 24h JSON-validation cooldown in separate store (negative)
- [ ] 4.4 Scenario: ollama 403 Pro slug → model lockout, free slug still routes
- [ ] 4.5 Scenario: structured rank native OpenRouter before prompt Ollama
- [ ] 4.6 Extend upstream-emulator ollama-cloud wire for prompt mode responses

## 5. Documentation and verification

- [ ] 5.1 Update `docs/providers.md` — Ollama Cloud native vs prompt JSON, weighted quota model
- [ ] 5.2 Update `provider-limits.yaml` notes with free vs Pro slug lists (2026-06-19 verification)
- [ ] 5.3 Add capability unit tests for delivery enum and ollama_cloud rank defaults
- [ ] 5.4 Coordinate with `per-model-quota-domain` remaining task (merge or complete before ship)
