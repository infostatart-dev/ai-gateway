## 1. Domain generalization (provider-agnostic)

- [x] 1.1 Audit `ladder_filter`, `pacing/registry`, `sort.rs` — confirm gated only on `quota_profile`, not `gemini` name
- [x] 1.2 Fix `quota_scope`: 402 unpaid slug → `Model` when `per-model` (before `PAYMENT_REQUIRED` → Project)
- [x] 1.3 Add `looks_like_unpaid_route(body)` helper; unit tests for never-purchased vs billing cap
- [x] 1.4 Extend `classify_429` with `free-models-per-day` pattern → `QuotaExhausted`
- [x] 1.5 Parse `X-RateLimit-Reset` header (epoch ms) into model cooldown duration
- [x] 1.6 Replace alphabetical model tie-break in `sort.rs` with `ladder_rank` for per-model providers
- [x] 1.7 Add `deprioritized` band support in `ModelLadderRegistry` + `ladder_rank.rs`

## 2. Catalog — OpenRouter per-model consumer

- [x] 2.1 Set `openrouter.quota-profile: per-model` in `provider-limits.yaml`
- [x] 2.2 Add explicit per-slug free tier limits (gpt-oss, nemotron, qwen, openrouter/free) with `rpd: 50`
- [x] 2.3 Add `provider-ladders.yaml` openrouter free bands: fast → capacity → stability → deprioritized
- [x] 2.4 Document per-slug vs shared-bucket correction in `docs/providers.md`
- [x] 2.5 Capture `openrouter-listmodels.json` fixture; add `catalog:verify-openrouter` test + mise task
- [x] 2.6 Wire `catalog:verify-openrouter` into `predeploy:rust` depends

## 3. Budget probe and mapper hardening

- [x] 3.1 On 402 Model scope: do not call slot-level `record_payment_required` that blocks `:free` routes
- [x] 3.2 Ensure `budget_probe_skips` runs before every OpenRouter hop (regression test)
- [x] 3.3 Regression: budget-aware dispatcher `model_id` overrides client `gpt-5.4-nano` on wire

## 4. Test pyramid (architectural use cases)

- [x] 4.1 Unit: `quota_scope` matrix — 402 unpaid Model, billing Project, free-models-per-day
- [x] 4.2 Unit: `pacing/scope` — separate gate keys for nemotron vs gpt-oss on same credential
- [x] 4.3 `routing_load`: `openrouter_nemotron_429_then_gpt_oss_200` (matrix A)
- [x] 4.4 `routing_load`: `openrouter_402_paid_does_not_kill_free` (matrix B)
- [x] 4.5 `intent_acceptance`: fast-thinking → gpt-oss stability before groq (matrix D)
- [x] 4.6 Re-run existing Gemini `routing_load` scenarios — no regression (matrix C)
- [x] 4.7 Emulator: `402-never-purchased` and `429-free-models-per-day` wire body profiles

## 5. Observability and release

- [x] 5.1 Log `exhaustion_scope` + `quota_profile` on OpenRouter classified failures (verify fields)
- [x] 5.2 CHANGELOG `[0.4.2-beta.4]` — unified quota-profile domain + OpenRouter fix
- [x] 5.3 Bump `Cargo.toml` workspace version to `0.4.2-beta.4`
- [ ] 5.4 Stage smoke: provider-stats OpenRouter `last_status_code: 200` when Gemini carries load

## 6. Validation

- [x] 6.1 `mise run predeploy:rust` green
- [x] 6.2 `cargo test -p ai-gateway --test routing_load --features testing` green
- [x] 6.3 `mise run openspec:validate per-model-quota-domain --strict`
