## Why

`chatgpt-web` on shared/datacenter egress hits OpenAI **“unusual activity”** risk flags quickly. The gateway then retries every **60s** (`provider-error` cooldown) while each attempt still fires **~8 upstream HTTP calls** (token, DPL, **3× warmup without cache**, 2× sentinel, conversation). That hammering prolongs IP/account blocks even when failover to other providers succeeds.

Release target after implementation and tests: **`0.3.0-beta.13`** (from `0.3.0-beta.12`).

## What Changes

- Add **session warmup cache** (60s TTL per cookie+token, bounded size) in `chatgpt-web` to skip redundant pre-sentinel GETs.
- **Invalidate warmup (and token) cache** on auth failures and abuse signals so a blocked session is not retried with stale “warm” state.
- **Lower embedded pacing** for `chatgpt-web`: **12 → 4 RPM**, **2 → 1 concurrent**, **3s → 12s** min interval (one in-flight completion, human-like spacing).
- Add **`abuse-block` cooldown** tier in provider limits and router cooldown config; apply it when upstream responses indicate OpenAI risk blocks (e.g. “unusual activity”) or sentinel hard blocks (502/403 mapped through the executor).
- Extend `cooldown_for_response` to classify abuse bodies on **502** (and preserve existing 429/auth paths).
- Harden **session-token rotation** tests (unchunked ↔ chunked NextAuth shapes) so Cloudflare helper cookies are never dropped on refresh.
- Unit and integration tests for warmup skip, cache invalidation, abuse classification, sentinel-block cooldown, and long cooldown duration for `chatgpt-web`.
- Document operational guidance in `docs/chatgpt-web.md` (stop retrying, browser sanity check, egress risk, pacing).

## Capabilities

### New Capabilities

- `chatgpt-web-stabilization`: Warmup cache, cache invalidation on block, abuse-block cooldown, session-cookie rotation tests, and router policy that prevents short retry loops on risk-flagged ChatGPT Web sessions.

### Modified Capabilities

- None (no living specs in `openspec/specs/` yet).

## Impact

- `crates/chatgpt-web/src/session/warmup.rs` (and small cache helper if split) — warmup TTL cache + invalidation hooks.
- `crates/chatgpt-web/src/session/exchange.rs`, `executor.rs` — clear caches on 401/403.
- `crates/chatgpt-web/src/session/cookie.rs` — rotation regression tests.
- `ai-gateway/src/router/retry_after/` — abuse body detection + `cooldown_for_response` branch.
- `ai-gateway/src/config/router_cooldown.rs` — new `abuse-block` duration field.
- `ai-gateway/config/embedded/provider-limits.yaml` — `chatgpt-web` pacing + `cooldown.abuse-block`.
- `crates/chatgpt-web/src/executor/tests.rs` — warmup cache call-count test.
- `ai-gateway/src/config/provider_limits.rs`, `ai-gateway/src/router/retry_after/mod.rs` — catalog + cooldown tests.
- `docs/chatgpt-web.md` — stabilization notes and ops playbook.
- Workspace version: root `Cargo.toml` → **`0.3.0-beta.13`**.
