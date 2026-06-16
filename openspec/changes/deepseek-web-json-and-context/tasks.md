## 1. Shared structured output

- [x] 1.1 Create `crates/web-structured-output` with parse, instruct, validate, retry suffix helpers extracted from `chatgpt-web/src/schema.rs`
- [x] 1.2 Migrate `chatgpt-web` to the shared module; run existing schema tests — zero behavior change
- [x] 1.3 Wire `ai-gateway/src/router/budget_aware/structured_output.rs` to the shared module

## 2. DeepSeek Web structured output

- [x] 2.1 Add `DeepSeekWebConverter` + registry entry (`OpenAI → deepseek-web OpenAICompatible`)
- [x] 2.2 Set `supports_json_schema: true` for **both** `deepseek-chat` and `deepseek-reasoner` in `providers.rs` and embedded `providers.yaml`
- [x] 2.3 Integrate schema parse/inject in executor final-turn path
- [x] 2.4 Validate **`content` only** on reasoner (ignore `reasoning_content` for schema check)
- [x] 2.5 Implement structured validation + `MAX_STRUCTURED_RETRIES = 2` on final turn
- [x] 2.6 Add `deepseek probe --structured-output` smoke command (diagnostic only, no config side effects)
- [x] 2.7 Add unit tests: strict schema on chat and reasoner, invalid JSON retry, autodefault eligibility mock

## 3. DeepSeek Web context chunking

- [x] 3.1 Set `DEEPSEEK_WEB_CONTEXT_TOKENS = 128_000` and `DEEPSEEK_UPLOAD_PAYLOAD_TOKENS = 45_000`; update embedded catalog from 65536
- [x] 3.2 Add `plan_completion_turns()` wrapping `plan_web_chunks` with DeepSeek budget profile
- [x] 3.3 Implement PoW cache (45s TTL, session-scoped) for multi-turn uploads
- [x] 3.4 Refactor executor: single token exchange, single session create, multi-turn loop, deferred session delete
- [x] 3.5 Implement `web_turn_to_prompt()`; retire `history_window` slicing on planner path
- [x] 3.6 Add `deepseek probe --context-limit` CLI (diagnostic)
- [x] 3.7 Add unit tests: large dossier → multi-part plan at 45k parts; PoW cache hit on turn 2 (mock)

## 4. Router, pacing, observability

- [x] 4.1 Confirm each executor turn acquires existing `deepseek-web` pacing permit
- [x] 4.2 Extend budget-aware route trace with turn count, upload parts, pow_cache_hits
- [x] 4.3 Add integration test: autodefault + json_schema + only deepseek credential (mock dispatch)

## 5. Documentation and release

- [x] 5.1 Update `docs/deepseek-web.md` — JSON schema (chat+reasoner), chunk upload, probes, PoW cache, limits table
- [x] 5.2 Update `docs/routing.md` — DeepSeek Web json_schema eligibility
- [x] 5.3 Add **`CHANGELOG.md` `## [0.3.0-beta.19]`** section (features listed in spec); bump `Cargo.toml` to `0.3.0-beta.19`
- [x] 5.4 Fix `dev/secrets.local.example.yaml` session-file path comment

## 6. Validation gate

- [x] 6.1 `mise exec -- openspec validate deepseek-web-json-and-context --strict`
- [x] 6.2 `cargo test -p deepseek-web --all-features`
- [x] 6.3 `cargo test -p chatgpt-web --all-features`
- [x] 6.4 Targeted gateway tests: mapper registry, capability, structured_output gate
- [x] 6.5 `cargo clippy` on touched crates
