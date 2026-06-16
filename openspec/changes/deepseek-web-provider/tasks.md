## 1. Catalog & operator surface (front-first)

- [ ] 1.1 Add `deepseek-web` block to embedded `providers.yaml` (models + model-capabilities: at minimum `deepseek-chat`, `deepseek-reasoner`; `supports-tools: false`)
- [ ] 1.2 Add `deepseek-web-default` credential slot to embedded `credentials.yaml` (session-file path, comment mirroring chatgpt/pplx)
- [ ] 1.3 Add `deepseek-web` block to `provider-limits.yaml` (single-session tier: `concurrent: 1`, `min-interval-ms`, `rpm`, cooldown rate-limit/auth-error)
- [ ] 1.4 Add `DEEPSEEK_BROWSER_CLI` to `.env.template` and document the credential slot in `docs/credentials.md`
- [ ] 1.5 Remove `perplexity-web` blocks from `providers.yaml` and `credentials.yaml` (keep crate/CLI/config in tree); update catalog tests/docs as needed

## 2. Shared browser-login: page-value capture

- [ ] 2.1 Add a generic page-value extractor to `web-browser-login` (run `localStorage.getItem(<key>)` via `page.evaluate`), returning the value when present
- [ ] 2.2 Add a `deepseek_domain` / login target helper and a poll variant that completes on token presence (reuse existing poll loop; keep cookie path intact)
- [ ] 2.3 Unit test: cookie capture path unchanged; token extractor returns value when key set

## 3. `deepseek-web` crate (port from OmniRoute)

- [ ] 3.1 Scaffold crate `crates/deepseek-web` mirroring `chatgpt-web` (Cargo.toml, lib.rs, feature `login`); add `sha3` dependency
- [ ] 3.2 `constants.rs`: base/API URLs, completion/session/pow/users endpoints, `FAKE_HEADERS`, `X-Ds-Pow-Response` header name
- [ ] 3.3 `errors.rs`: `Error` enum (SessionAuth, Upstream{status,message}, Other) matching chatgpt-web error mapping
- [ ] 3.4 `pow.rs`: `DeepSeekHashV1` solver (SHA3-256 over `"{salt}_{expire_at}_{nonce}"`) + challenge struct + base64 answer encoder
- [ ] 3.5 `session/file.rs` + `session/token.rs`: session file `{token, saved_at}`, normalize JSON-wrapped `{"value":...}`, in-process access-token cache
- [ ] 3.6 `tls/client.rs`: HTTP client with browser-like headers (follow chatgpt-web tls module)
- [ ] 3.7 `token`/`session`/`pow_challenge` calls: `users/current`, `chat_session/create`, `chat/create_pow_challenge` (+ delete cleanup)
- [ ] 3.8 `completion.rs`: build completion request body + headers (model_type, thinking/search flags, prompt assembly from messages)
- [ ] 3.9 `sse.rs`: translate DeepSeek `p`/`o`/`v` op-stream (THINK→reasoning_content, ANSWER→content, search_results→citations) into OpenAI chunks / aggregated body
- [ ] 3.10 `executor.rs`: orchestrate token → session → pow → completion → translate; expose `Executor::execute(ExecuteRequest)` analogous to chatgpt-web
- [ ] 3.11 `login.rs` (feature `login`): headed-browser capture of `userToken` + import fallback; `run_login` / `save_session_from_token`

## 4. Gateway integration

- [ ] 4.1 `config/deepseek_web.rs`: `SESSION_ENV=DEEPSEEK_BROWSER_CLI`, `is_deepseek_web`, `session_path_for_credential`, `session_file_available`, `load_session_token`
- [ ] 4.2 `types/provider.rs` / `ProviderKey::from_env`: treat `deepseek-web` as session-file (NotRequired) like chatgpt-web
- [ ] 4.3 `config/credentials.rs`: discover `deepseek-web` session credential (extend `fill_session_credentials` or generalize)
- [ ] 4.4 `router/pacing/scope.rs`: key the gate scope by the deepseek-web session path
- [ ] 4.5 `dispatcher/deepseek_web.rs`: `dispatch_deepseek_web` building `DispatchOutcome` via `outcome_from_bytes`; error mapping to OpenAI error JSON
- [ ] 4.6 `dispatcher/service/dispatch.rs`: add the `is_deepseek_web` branch alongside chatgpt-web
- [ ] 4.7 `cli/mod.rs` + `cli/deepseek_login.rs`: wire `deepseek login` / import subcommands

## 5. Tests & verification

- [ ] 5.1 PoW test vector (known salt/expire_at/challenge → answer) ported from OmniRoute reference
- [ ] 5.2 SSE translation tests: thinking/answer fragments, search citations, `[DONE]` termination (port OmniRoute cases)
- [ ] 5.3 Session/token tests: JSON-wrapped token normalization, access-token cache expiry, 401 → SessionAuth mapping
- [ ] 5.4 Dispatcher/credential tests: provider hidden without session file; pacing scope keyed by path
- [ ] 5.5 End-to-end smoke through the existing test harness (confirm the gpt5-nano wrapper path works for a `deepseek-web` model); update embedded-config catalog tests
- [ ] 5.6 `openspec validate --strict` clean; `cargo clippy` + targeted `cargo test` green per pre-deploy skill
