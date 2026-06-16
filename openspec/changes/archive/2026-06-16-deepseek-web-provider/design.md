## Context

- `chatgpt-web` is the reference web-session provider (dispatcher, pacing, limits,
  session CLI).
- `perplexity-web` has a crate but **no dispatcher branch** — not usable for inference.
- **R&D complete (2026-06-16):** minimal `deepseek-web` crate + `deepseek login` /
  `import` CLI; `web-browser-login` polls `localStorage.userToken` on
  `chat.deepseek.com`. Verified session file at `dev/deepseek-session.json`.
- Release version for this change: **`0.3.0-beta.14`**.

## Protocol (reverse-engineered)

- Base `https://chat.deepseek.com`, API `https://chat.deepseek.com/api`.
- **Credential**: long-lived `userToken` in `localStorage.userToken` (raw string or
  JSON `{"value":"..."}`).
- **Access token**: `GET /api/v0/users/current` with `Authorization: Bearer <userToken>`
  → `data.biz_data.token` (~1h cache in-process). 401/403 ⇒ re-login.
- **Session**: `POST /api/v0/chat_session/create` → `chat_session.id`; optional
  `POST /api/v0/chat_session/delete` cleanup.
- **PoW**: `POST /api/v0/chat/create_pow_challenge` with
  `{"target_path":"/api/v0/chat/completion"}` → challenge
  `{algorithm:"DeepSeekHashV1", challenge, salt, signature, difficulty, expire_at}`.
  Solve: SHA3-256 over `"{salt}_{expire_at}_{nonce}"`; first `nonce` where hex
  digest equals `challenge`. Send base64 JSON answer in `X-Ds-Pow-Response`.
- **Completion**: `POST /api/v0/chat/completion` with Bearer accessToken, PoW header,
  browser-like `X-*` client headers, fake `Cookie`, body
  `{chat_session_id, parent_message_id, model_type, prompt, thinking_enabled,
  search_enabled, ...}`.
- **Response**: SSE `p`/`o`/`v` ops — `THINK` → `reasoning_content`, `ANSWER` →
  `content`, search results → citations.

## Already implemented (R&D — do not redo)

| Area | Status |
|------|--------|
| `web-browser-login` `poll_local_storage_value_with_options` | Done; handles empty `localStorage` without aborting poll |
| `deepseek_domain` / `deepseek_ready_url` helpers | Done |
| `crates/deepseek-web` session file `{token, saved_at}` + `normalize_user_token` | Done |
| `login.rs` + `deepseek login` / `import` in `main.rs` | Done |
| `deepseek-login` Cargo feature | Done |
| PoW, executor, tls, sse, dispatcher, catalog | **Not done** |

## Goals / Non-Goals

**Goals:**

- End-to-end `deepseek-web` through `/v1/chat/completions` (stream + non-stream).
- Native SHA3 PoW; small modules (~60 lines each where practical).
- Conservative pacing from day one (single session, browser-like rate).
- R&D smoke: `users/current` + one completion against live API using saved session
  (manual or `deepseek probe` CLI).
- Disable perplexity in catalog only.

**Non-Goals:**

- WASM PoW runtime.
- Native tools round-trips (`supports-tools: false` initially).
- File upload (`ref_file_ids`).
- Removing `perplexity-web` crate/CLI.

## Decisions

### D1: PoW in Rust with `sha3`

`DeepSeekHashV1` = SHA3-256 sponge over `"{salt}_{expire_at}_{nonce}"`. Native
`sha3` crate; pin with a known challenge→answer unit test.

### D2: Credential = persisted `userToken`

Session file only stores `userToken`; `accessToken` derived per request and cached
in-process (~1h). Login/import already implemented.

### D3: Crate layout (extend existing scaffold)

Add to `crates/deepseek-web`: `pow`, `tls`, `completion`, `sse`, `executor`;
expand `constants` with API URLs and client headers.

### D4: Dispatcher branch

`dispatch_deepseek_web` parallel to `chatgpt_web`; `outcome_from_bytes`; map
401/403 → `invalid_session` + auth-error cooldown.

### D5: Models

Minimum catalog:

- `deepseek-web/deepseek-chat` — `thinking_enabled=false`
- `deepseek-web/deepseek-reasoner` — `thinking_enabled=true`

Optional follow-up: `-search` / expert variants via model name or body flags.

### D6: Pacing (conservative single session)

Apply from initial ship — same philosophy as `chatgpt-web-stabilization`:

| Knob | Value | Rationale |
|------|-------|-----------|
| `rpm` | **6** | Free web tier; slightly higher than ChatGPT web (no sentinel warmup chain) but still human-like |
| `concurrent` | **1** | One in-flight completion per session |
| `min-interval-ms` | **10000** | ≥10s between paced starts |
| `cooldown.rate-limit` | **120s** | After HTTP 429 |
| `cooldown.auth-error` | **30m** | After 401/403 on token exchange |

Each completion ≈ 4 upstream calls (users/current cache miss, session create, pow,
completion) — pacing limits **completion starts**, not raw HTTP.

### D7: Perplexity catalog disable

Remove `perplexity-web` from embedded `providers.yaml` and `credentials.yaml` only.

### D8: R&D verification gate (before full executor merge)

Using `dev/deepseek-session.json`:

1. `GET /api/v0/users/current` with Bearer `userToken` → access token.
2. Optional: one minimal completion smoke (non-stream) before wiring dispatcher.

Implement as `deepseek probe` CLI subcommand (like perplexity probe) or documented
`curl` in `docs/deepseek-web.md`.

## Risks / Trade-offs

- **PoW SHA3 variant mismatch** → unit test vector before network path.
- **Token expiry** → auth-error cooldown + `invalid_session`; re-run `deepseek login`.
- **Free-tier rate limits** → D6 pacing; tune after live smoke.
- **Browser login flakiness** → `import --token` fallback (already works).

## Migration Plan

1. Finish executor + gateway + catalog (tasks below).
2. Bump **`0.3.0-beta.13` → `0.3.0-beta.14`**.
3. Set `DEEPSEEK_BROWSER_CLI=dev/deepseek-session.json` (or credential slot path).
4. Rollback: remove catalog entries; old binary ignores deepseek-web.

## Open Questions

- Add `abuse-block` cooldown for DeepSeek risk messages? Defer unless smoke shows
  short `provider-error` retry storms.
- Session reuse vs fresh `chat_session/create` per request — start with fresh per
  completion (simpler); add rolling cache if needed.
