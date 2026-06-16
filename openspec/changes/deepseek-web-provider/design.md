## Context

The gateway already runs one web-session provider end-to-end: `chatgpt-web`
(crate `chatgpt-web`, `dispatcher/chatgpt_web.rs`, `config/chatgpt_web.rs`, pacing
scope, provider limits, `chatgpt login` CLI). `perplexity-web` exists as a crate but
has **no dispatcher branch**, so it cannot serve inference; it also has Cloudflare
cookie-auth problems. `deepseek-web` does not exist yet.

A complete, working reference implementation of the DeepSeek web protocol lives in
the repo at `bindings/OmniRoute/open-sse` (TypeScript): `executors/deepseek-web.ts`,
`lib/deepseek-pow.ts`, `lib/deepseek-pow-solver.cjs`, `lib/sha3_wasm_bg.wasm`,
`services/deepseekQuotaFetcher.ts`, plus unit tests. This design ports that protocol
to Rust following the `chatgpt-web` shape.

**Reverse-engineered protocol (from OmniRoute):**

- Base `https://chat.deepseek.com`, API `https://chat.deepseek.com/api`.
- **Credential**: long-lived `userToken` read from `localStorage.userToken` on
  chat.deepseek.com (stored as JSON `{"value":"..."}`; raw string also accepted).
- **Access token**: `GET /api/v0/users/current` with `Authorization: Bearer <userToken>`
  returns `data.biz_data.token` — a short-lived `accessToken` (cache ~1h). 401/403 ⇒
  token expired.
- **Session**: `POST /api/v0/chat_session/create` (Bearer accessToken) ⇒
  `data.biz_data.chat_session.id`. Optional reuse (rolling-window memory) vs
  fresh-per-request. `POST /api/v0/chat_session/delete` for cleanup.
- **PoW**: `POST /api/v0/chat/create_pow_challenge` body
  `{"target_path":"/api/v0/chat/completion"}` ⇒ `data.biz_data.challenge`
  `{algorithm:"DeepSeekHashV1", challenge, salt, signature, difficulty, expire_at,
  target_path}`. Solve, then send the answer in header `X-Ds-Pow-Response` as
  base64(JSON `{algorithm, challenge, salt, answer, signature, target_path}`).
- **PoW algorithm `DeepSeekHashV1`**: SHA3-256 sponge. For nonce in
  `0..difficulty`, compute `SHA3-256("{salt}_{expire_at}_{nonce}")`; the answer is
  the first `nonce` whose hex digest equals `challenge`. (OmniRoute uses a WASM
  solver for speed and a pure-JS SHA3 fallback that proves the algorithm.)
- **Completion**: `POST /api/v0/chat/completion` with headers Bearer accessToken,
  `X-Ds-Pow-Response`, `X-Client-Timezone-Offset`, fake `Cookie`, and the `X-*`
  client headers. Body: `{chat_session_id, parent_message_id:null, model_type,
  prompt, ref_file_ids, thinking_enabled, search_enabled, preempt:false}`.
- **Response**: SSE in DeepSeek's `p`/`o`/`v` op-protocol. Fragments typed
  `THINK` → OpenAI `reasoning_content`, `ANSWER`/`RESPONSE` → `content`;
  `response/search_results` accumulate citations. Translate to
  `chat.completion.chunk` / `chat.completion`.

## Goals / Non-Goals

**Goals:**
- Production `deepseek-web` provider reachable through the standard
  OpenAI-compatible `/v1/chat/completions` path, both streaming and non-streaming.
- Port PoW, token exchange, session, and SSE translation to Rust with small,
  single-responsibility modules (≤~60 lines each; OOP/structs where it helps).
- `deepseek login` headed-browser flow that captures `localStorage.userToken`,
  reusing the shared `web-browser-login` crate (extended generically).
- Per-session pacing: free-tier concurrency/min-interval/cooldown via
  `provider-limits.yaml`, matching the `chatgpt-web` mechanism.
- Disable `perplexity-web` in the catalog without deleting its code.

**Non-Goals:**
- No WASM runtime in Rust (PoW done with a native SHA3 crate).
- No native `tools`/function-calling round trips beyond what OmniRoute does
  (prompt-serialized tools may be a follow-up; initial models can be
  `supports-tools: false` like `chatgpt-web`).
- No file upload (`ref_file_ids`) in v1.
- Not removing the `perplexity-web` crate/CLI.

## Decisions

### D1: PoW in Rust with `sha3`, no WASM
`DeepSeekHashV1` is plain SHA3-256 over `"{salt}_{expire_at}_{nonce}"`. We use the
`sha3` crate and a tight nonce loop. Difficulty (~144000) means microseconds-to-low-
ms in native Rust — far faster than the JS fallback and on par with WASM, without an
embedded `WebAssembly` runtime. Alternative (embed `sha3_wasm_bg.wasm` + a wasm
runtime) rejected: needless dependency and complexity for a known hash.

### D2: Credential = `userToken`, captured from localStorage
Unlike chatgpt-web/perplexity-web (cookie sessions), DeepSeek auth is a bearer
`userToken` in `localStorage`. We extend `web-browser-login` with a generic
"page value extractor" (`page.evaluate("localStorage.getItem('userToken')")`) and
add a `deepseek login` command that polls until the token is present, then writes a
session file `{ "token": "...", "saved_at": ... }`. A `--token`/`--cookie` import
fallback mirrors chatgpt/pplx. The `accessToken` is derived at request time and
cached in-process; only the `userToken` is persisted.

### D3: Crate `deepseek-web` mirrors `chatgpt-web` layout
Modules: `constants`, `errors`, `session` (file + token), `tls` (client),
`pow` (sha3 solver + challenge fetch + header encode), `completion` (request build),
`sse` (p/o/v → OpenAI), `executor` (orchestration: token → session → pow →
completion → translate), `login` (feature-gated). Keeps each file small and mirrors
the working chatgpt-web shape for reviewability.

### D4: Dispatcher branch parallel to chatgpt-web
Add `is_deepseek_web(provider)` and a `dispatch_deepseek_web` branch in
`dispatcher/service/dispatch.rs` (the same place that special-cases chatgpt-web),
returning a `DispatchOutcome` via `outcome_from_bytes`. Credential discovery gets a
`deepseek-web` session-file path (like `fill_session_credentials` for chatgpt-web),
and `pacing/scope.rs` keys the gate by the session-file path.

### D5: Models & options surface
Catalog models map to DeepSeek options via name/body, following OmniRoute's
`resolveModelOptions`:
- `deepseek-chat` → `model_type=default`, `thinking_enabled=false`.
- `deepseek-reasoner` → `thinking_enabled=true` (DeepThink/R1).
- `expert`/`pro` in name → `model_type=expert`; `search` in name or
  `search_enabled` in body → web search. Exact model list finalized in tasks; expose
  both chat and reasoner at minimum.

### D6: Pacing for a free single session
Add a `deepseek-web` block to `provider-limits.yaml` with a conservative
single-session tier (start: `concurrent: 1`, a `min-interval-ms`, modest `rpm`, and
cooldowns for rate-limit/auth-error) so the gateway serializes calls and backs off,
respecting DeepSeek free limits. Values are heuristic and tunable.

### D7: Perplexity disabled via catalog only
Remove `perplexity-web` from embedded `providers.yaml` and `credentials.yaml` so it
is neither advertised nor discovered. Keep the crate, `config/perplexity_web.rs`,
`cli/perplexity_login.rs`, and pacing-scope handling intact (dead but compiling) for
a clean future re-enable. No behavior change for other providers.

## Risks / Trade-offs

- [DeepSeek changes PoW/headers/SSE shape] → Isolate protocol constants and the
  SSE parser; cover with ported unit tests (PoW vector, SSE fragments) so breakage
  surfaces fast. The OmniRoute reference stays in-tree as the source of truth.
- [SHA3 variant/format mismatch (Keccak vs SHA3, padding)] → Pin behavior with a
  known challenge/answer test vector captured from the reference before wiring the
  network path.
- [`userToken` expiry / silent 401] → Map 401/403 from `users/current` and
  completion to a gateway auth error + auth-error cooldown, surfaced as
  `invalid_session` so the operator re-runs `deepseek login`.
- [Free-tier rate limits / concurrency bans] → Conservative pacing (D6) and
  cooldowns; per-session gate keyed on the session-file path.
- [Browser automation flakiness for token capture] → Keep the import fallback so
  the provider is usable even if headed login fails in an environment.

## Migration Plan

Additive. New provider is inert until `DEEPSEEK_BROWSER_CLI` (or the credential
slot) points at a valid session file. Disabling Perplexity only removes catalog
entries; rollback = restore the two yaml blocks. No data migration.

## Open Questions

- Final model slugs for the catalog (`deepseek-chat`/`deepseek-reasoner` plus any
  `-search`/`-expert` variants) — resolve in tasks.
- Whether to port OmniRoute's prompt-serialized tool-calling now or as a follow-up
  (initial plan: `supports-tools: false`).
