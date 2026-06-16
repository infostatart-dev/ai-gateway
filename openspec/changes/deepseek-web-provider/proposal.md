## Why

We want three browser-session ("web") providers behind the gateway. `chatgpt-web`
works end-to-end. `perplexity-web` is only half-wired (no dispatcher path, plus
Cloudflare/cookie auth problems) and is not usable for inference. `deepseek-web`
(chat.deepseek.com) is free, fast, and uses a bearer `userToken` + per-request
PoW — a good next web provider after ChatGPT.

R&D already validated headed-browser login: `deepseek login` captures
`localStorage.userToken` into a session file. This change completes the provider
(executor, dispatcher, catalog, pacing) and parks Perplexity in the catalog.

Release target after implementation and tests: **`0.3.0-beta.14`** (from
`0.3.0-beta.13`).

## What Changes

- **NEW** `deepseek-web` provider: OpenAI-compatible models from chat.deepseek.com
  (`/api/v0/...`), including `DeepSeekHashV1` PoW, `userToken`→`accessToken`
  exchange, session creation, and DeepSeek→OpenAI SSE translation.
- **DONE (R&D)** `deepseek login` / `deepseek import --token` CLI and minimal
  `deepseek-web` session crate; extend `web-browser-login` with `localStorage`
  polling (including empty-value retry).
- **MODIFIED** dispatcher: `deepseek-web` execution branch with credential
  discovery, pacing scope, and conservative single-session limits.
- Disable `perplexity-web` in the catalog (crate retained, dormant).
- Docs: `docs/deepseek-web.md`, `.env.template` (`DEEPSEEK_BROWSER_CLI`).

## Capabilities

### New Capabilities

- `deepseek-web-provider`: Web-session provider for DeepSeek chat/reasoner models
  with PoW, token exchange, SSE translation, pacing, and gateway integration.
- `web-session-token-login`: Headed-browser capture of page-scoped credentials
  (`localStorage.userToken`) — partially implemented; finish tests/docs in this
  change.

### Modified Capabilities

- None (no living specs in `openspec/specs/` yet).

## Impact

- Crates: `crates/deepseek-web` (expand beyond login/session); `web-browser-login`
  (tests for storage poll).
- Gateway: dispatcher, config, credentials, pacing scope, CLI (already wired for login).
- Config: `providers.yaml`, `credentials.yaml`, `provider-limits.yaml`.
- Dependency: `sha3` for native PoW (no WASM runtime).
- Workspace version: **`0.3.0-beta.14`**.
