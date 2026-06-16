## Why

We want three browser-session ("web") providers behind the gateway. `chatgpt-web`
works end-to-end. `perplexity-web` is only half-wired (no dispatcher path, plus
Cloudflare/cookie auth problems) and is not usable for inference. `deepseek-web`
(chat.deepseek.com) is free, fast, and fully reverse-engineered in the in-tree
`bindings/OmniRoute` reference — making it the highest-value web provider to add now.

This change adds a production `deepseek-web` provider and parks Perplexity (kept in
the tree, disabled in the catalog) so routing only advertises providers that work.

## What Changes

- **NEW** `deepseek-web` provider: OpenAI-compatible models served from
  chat.deepseek.com via the web API (`/api/v0/...`), including the `DeepSeekHashV1`
  proof-of-work, `userToken`→`accessToken` exchange, session creation, and
  DeepSeek→OpenAI SSE translation (reasoning/answer/search).
- **NEW** `deepseek login` CLI: headed-browser login that captures the
  `localStorage.userToken` from chat.deepseek.com into a session file, plus a
  `--cookie`/token import fallback.
- **MODIFIED** shared `web-browser-login` crate: add a generic page-value extractor
  (run JS in the page to read `localStorage`), so a login flow can capture a token,
  not only cookies. Existing cookie capture for `chatgpt-web`/`perplexity-web` is
  unchanged.
- **MODIFIED** dispatcher: add a `deepseek-web` execution branch (alongside the
  existing `chatgpt-web` branch), with credential discovery, pacing scope, and
  provider limits (free tier: low concurrency + min-interval + cooldowns).
- Disable `perplexity-web` in the catalog: remove its entries from embedded
  `providers.yaml` and `credentials.yaml`. The `perplexity-web` crate, CLI, and
  config helpers stay in the tree for future re-enablement (no code deletion).

## Capabilities

### New Capabilities
- `deepseek-web-provider`: A web-session provider that serves DeepSeek models
  through chat.deepseek.com, handling PoW auth, session lifecycle, model options
  (thinking/search/expert), and OpenAI-compatible request/response translation.
- `web-session-token-login`: Headed-browser login capability that captures a
  page-scoped credential value (e.g. `localStorage.userToken`) into a session
  file, generalizing the existing cookie-only login flow for web providers.

### Modified Capabilities
<!-- No existing openspec specs to modify; perplexity disable is a catalog/config change, not a spec requirement change. -->

## Impact

- Crates: new `deepseek-web` crate; modified `web-browser-login`.
- Gateway: `ai-gateway/src/dispatcher/` (new `deepseek_web.rs` + dispatch branch),
  `ai-gateway/src/config/` (new `deepseek_web.rs`), `ai-gateway/src/cli/`
  (new `deepseek_login.rs`), `router/pacing/scope.rs`, `config/credentials.rs`.
- Config: `config/embedded/providers.yaml`, `credentials.yaml`,
  `provider-limits.yaml`; `.env.template` (`DEEPSEEK_BROWSER_CLI`).
- Perplexity removed from advertised catalog (crate retained, dormant).
- New dependency: a SHA3 implementation (`sha3`/`tiny-keccak`) for the PoW solver;
  no WASM runtime required.
