## 1. Credential catalog

- [x] 1.1 Add `cost-class` to `CredentialSpec` with derivation (`free` / `paid` / `paid-browser`)
- [x] 1.2 Annotate slots in `credentials.yaml`; add `chatgpt-web-default` if missing
- [x] 1.3 Map `deepseek-web-default` → `free`, `chatgpt-web-default` → `paid-browser`

## 2. Ranking and autodefault order

- [x] 2.1 Refactor `default_provider_budget_rank` to cost-class bands + ordered list
- [x] 2.2 Update `autodefault_provider_order()` — chatgpt-web last, deepseek-web after gemini free band
- [x] 2.3 Integrate cost-class into candidate sort before budget-rank
- [x] 2.4 Update `docs/routing.md` with new priority list and cost-class table

## 3. Model binding

- [x] 3.1 Reorder `gpt-5.4-nano` in `model-mapping.yaml` per design D4
- [x] 3.2 Audit `gpt-5.4-mini` for cost-first ordering
- [x] 3.3 Add regression test: nano mapping free openrouter before anthropic

## 4. CLI and docs

- [x] 4.1 Autodefault curl example → `openai/gpt-5.4-nano` in `cli/helpers.rs`
- [x] 4.2 Optional `AI_GATEWAY_AUTODEFAULT_DEFAULT_MODEL` env override
- [x] 4.3 Document ChatGPT Web last-resort and DeepSeek Web placement in `docs/credentials.md`

## 5. Tests

- [x] 5.1 Cost-class derivation tests
- [x] 5.2 Free API ranks before chatgpt-web; paid API before chatgpt-web
- [x] 5.3 Gemini free before deepseek-web; deepseek-web before gemini-default
- [x] 5.4 Update `autodefault_scenario_tests` fixture order
- [x] 5.5 Update `default_provider_budget_order_matches_autodefault_policy` test
- [x] 5.6 `json_schema_required`: cost-class still beats `json_schema_rank` tiebreak
- [x] 5.7 `tools_required`: deepseek-web excluded; free API or paid path before chatgpt-web

## 6. Validation and release

- [x] 6.1 `mise exec -- openspec validate autodefault-routing-priority --strict`
- [x] 6.2 Targeted Rust tests (rank, read, model-mapping)
- [x] 6.3 Bump `Cargo.toml` **`0.3.0-beta.16` → `0.3.0-beta.17`** (after CI green;
      version may lag code — beta.16 already at `0.3.0-beta.16`)

## 7. CHANGELOG catch-up (beta.12–17)

`CHANGELOG.md` stops at **`0.3.0-beta.11`** while code shipped beta.12–16.
Backfill in one pass when closing beta.17 — do not leave gaps.

- [x] 7.1 **`0.3.0-beta.12`** — gemini free multi-account (four slots, round-robin)
- [x] 7.2 **`0.3.0-beta.13`** — chatgpt-web stabilization (warmup cache, abuse-block 4h,
      pacing 4 rpm / 12s / 1 concurrent)
- [x] 7.3 **`0.3.0-beta.14`** — deepseek-web browser-session provider (PoW, SSE)
- [x] 7.4 **`0.3.0-beta.15`** — github-models PAT provider
- [x] 7.5 **`0.3.0-beta.16`** — payload-aware autodefault routing (token estimate,
      TPM/context filter, Gemini quota sibling skip, `json_schema_rank`, route trace)
- [x] 7.6 **`0.3.0-beta.17`** — autodefault cost-class priority (this change);
      call out ChatGPT Web last-resort breaking change for browser-first operators
