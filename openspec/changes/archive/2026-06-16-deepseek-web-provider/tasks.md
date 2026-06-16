## 1. Catalog & operator surface

- [x] 1.1 Add `deepseek-web` block to embedded `providers.yaml` (`deepseek-chat`, `deepseek-reasoner`; `supports-tools: false`)
- [x] 1.2 Add `deepseek-web-default` credential slot to embedded `credentials.yaml` (session-file path)
- [x] 1.3 Add `deepseek-web` block to `provider-limits.yaml`: **`rpm: 6`**, **`concurrent: 1`**, **`min-interval-ms: 10000`**, cooldowns `rate-limit: 120s`, `auth-error: 30m`
- [x] 1.4 Add `DEEPSEEK_BROWSER_CLI` to `.env.template`; add `docs/deepseek-web.md` and update `docs/credentials.md` / `docs/providers.md`
- [x] 1.5 Remove `perplexity-web` blocks from `providers.yaml` and `credentials.yaml` (keep crate/CLI); update catalog tests

## 2. Shared browser-login (mostly done)

- [x] 2.1 `poll_local_storage_value_with_options` in `web-browser-login` (`poll_storage.rs`)
- [x] 2.2 `deepseek_domain` / `deepseek_ready_url` helpers in `config.rs`
- [x] 2.3 Unit test: empty `localStorage` does not abort poll; cookie capture path unchanged (regression)

## 3. `deepseek-web` crate — session & login (R&D done)

- [x] 3.1 Scaffold crate `crates/deepseek-web` (workspace member, `login` feature)
- [x] 3.5 `session/file.rs` + `session/token.rs` (`{token, saved_at}`, JSON `{"value":...}` unwrap)
- [x] 3.11 `login.rs`: `run_login` / `save_session_from_token`
- [x] 3.2 Expand `constants.rs`: API URLs, endpoints, `FAKE_HEADERS`, `X-Ds-Pow-Response`, client `X-*` headers
- [x] 3.3 `errors.rs`: add `SessionAuth`, `Upstream { status, message }` (align with chatgpt-web mapping)
- [x] 3.4 `pow.rs`: `DeepSeekHashV1` solver (`sha3`) + challenge struct + base64 answer encoder
- [x] 3.6 `tls/fetch.rs`: HTTP client with browser-like headers (mirror `chatgpt-web` tls module)
- [x] 3.7 API calls: `users/current`, `chat_session/create`, `chat/create_pow_challenge`, optional `chat_session/delete`
- [x] 3.8 `completion.rs`: request body + headers (model_type, thinking/search flags, prompt from messages)
- [x] 3.9 `sse.rs`: `p`/`o`/`v` → OpenAI chunks / aggregated JSON
- [x] 3.10 `executor.rs`: token → session → pow → completion → translate; `Executor::execute`

## 4. Gateway integration

- [x] 4.1 `config/deepseek_web.rs`: `SESSION_ENV`, `is_deepseek_web`, session path helpers, `load_session_token`
- [x] 4.2 `types/provider.rs`: `deepseek-web` as session-file credential
- [x] 4.3 `config/credentials.rs`: discover `deepseek-web-default` when session file exists
- [x] 4.4 `router/pacing/scope.rs`: gate scope keyed by deepseek session path
- [x] 4.5 `dispatcher/deepseek_web.rs` + error mapping
- [x] 4.6 `dispatcher/service/dispatch.rs`: `is_deepseek_web` branch
- [x] 4.7 `cli/deepseek_login.rs` + `main.rs` subcommands + `deepseek-login` feature

## 5. R&D smoke & tests

- [x] 5.0 **R&D gate:** `deepseek probe` (or documented curl) — `users/current` with `dev/deepseek-session.json`; optional one non-stream completion
- [x] 5.1 PoW test vector (known salt/expire_at/challenge → answer)
- [x] 5.2 SSE translation tests (THINK, ANSWER, search citations, `[DONE]`)
- [x] 5.3 Session/token tests: JSON wrapper, access-token cache TTL, 401 → SessionAuth
- [x] 5.4 Dispatcher/credential tests: hidden without session; pacing scope by path
- [x] 5.5 Catalog tests for `deepseek-web` in embedded config
- [x] 5.6 `openspec validate deepseek-web-provider --strict`; `cargo clippy` + targeted `cargo test`

## 6. Version bump (`0.3.0-beta.14`)

- [x] 6.1 Bump root `Cargo.toml` **`0.3.0-beta.13` → `0.3.0-beta.14`**
- [x] 6.2 Confirm CI green
