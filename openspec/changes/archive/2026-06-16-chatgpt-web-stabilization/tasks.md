## 1. Warmup cache (`chatgpt-web` crate)

- [x] 1.1 Add warmup cache module in `crates/chatgpt-web/src/session/` (inline in `warmup.rs` or `warmup/cache.rs`): key = `cookie_key` + token suffix, TTL **60s**, max **200** entries, evict oldest
- [x] 1.2 Gate `run_session_warmup` HTTP loop on cache hit/miss; export `clear_warmup_cache()` and `invalidate_warmup_cache(cookie, access_token)` for tests/runtime
- [x] 1.3 Unit test: cache miss performs 3 fetches; immediate second call performs **0** warmup fetches
- [x] 1.4 Unit test: after TTL (mock clock or manual clear + time advance helper), warmup runs again
- [x] 1.5 Update `executor/tests.rs`: assert MockFetch call sequence — first `execute` includes 3 warmup responses; second `execute` within TTL skips them
- [x] 1.6 On 401/403 in `exchange.rs`, `sentinel/prepare.rs`, and `executor.rs` conversation path: call `invalidate_warmup_cache` alongside existing `invalidate_token_cache`

## 2. Session cookie rotation tests

- [x] 2.1 Add `merge_refreshed_cookie` test: unchunked token → chunked Set-Cookie (`.0`/`.1`); assert stale unchunked name absent
- [x] 2.2 Add test: rotation preserves `cf_clearance` (and `__cf_bm` if present) from original blob

## 3. Pacing (`provider-limits.yaml`)

- [x] 3.1 Lower `chatgpt-web` tier `plus-single-session` limits: **`rpm: 12 → 4`**, **`concurrent: 2 → 1`**, **`min-interval-ms: 3000 → 12000`**
- [x] 3.2 Optional: `chatgpt-web.cooldown.rate-limit: 120s → 180s` after HTTP 429
- [x] 3.3 Update `provider_limits.rs` test `catalog_contains_chatgpt_web_session_limits` for new pacing values
- [x] 3.4 Update `pacing/limits.rs` test `chatgpt_web_catalog_exposes_session_pacing` for **4 / 1 / 12s**

## 4. Abuse-block cooldown config

- [x] 4.1 Add `abuse_block` to `RouterCooldownConfig` and `ProviderCooldownOverrides` in `ai-gateway/src/config/router_cooldown.rs` (YAML key `abuse-block`, default **2h**)
- [x] 4.2 Add `cooldown-defaults.abuse-block: 2h` and `chatgpt-web.cooldown.abuse-block: 4h` in `ai-gateway/config/embedded/provider-limits.yaml`
- [x] 4.3 Extend `provider_limits.rs` tests: merged `chatgpt-web` cooldown includes `abuse_block == 4h`

## 5. Abuse classification + cooldown routing

- [x] 5.1 Add `looks_like_abuse_block` in `ai-gateway/src/router/retry_after/abuse.rs` with rules from design (unusual activity, sentinel+blocked, guarded “try again later”)
- [x] 5.2 Tests in `abuse.rs`: positive (OpenAI unusual-activity string, sentinel-block message), negative (generic 502, plain “try again later”)
- [x] 5.3 Extend `cooldown_for_response` in `retry_after/mod.rs`: for **502**/**503**, buffer body once; if `looks_like_abuse_block`, return `abuse_block + retry_after_buffer`
- [x] 5.4 Test in `retry_after/mod.rs`: 502 + unusual-activity JSON body → **4h + buffer** with `chatgpt-web` merged config; generic 502 → **60s + buffer**
- [x] 5.5 Executor test or integration test: simulated 401/403 clears warmup so third execute within TTL still warms up

## 6. Docs

- [x] 6.1 Update `docs/chatgpt-web.md`: warmup cache, **pacing (4 rpm / 12s / 1 concurrent)**, cache invalidation on auth/block, abuse-block cooldown, ops playbook

## 7. Version bump and release (`0.3.0-beta.13`)

- [x] 7.1 Run scoped tests: `cargo test -p chatgpt-web`, `cargo test retry_after`, `cargo test provider_limits`, `cargo test pacing`, relevant `executor` and `cookie` tests
- [x] 7.2 Run `cargo clippy` on touched crates
- [x] 7.3 Bump root `Cargo.toml` workspace version **`0.3.0-beta.12` → `0.3.0-beta.13`**
- [x] 7.4 `mise exec -- openspec validate chatgpt-web-stabilization --strict`
- [x] 7.5 Confirm CI passes (Rust tests + Docker publish per existing workflows)
