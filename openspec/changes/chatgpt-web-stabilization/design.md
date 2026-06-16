## Context

- Each `chatgpt-web` completion today: `exchange_session` → `fetch_dpl` → **`run_session_warmup` (3 GET, always)** → sentinel prepare + chat-requirements → optional PoW → conversation POST.
- Warmup mimics a browser page load (`/me`, `/conversations`, `/models`); repeating it on every completion adds avoidable upstream load and increases risk-flag surface.
- Upstream “unusual activity” arrives as **502** with plain-text/JSON body through `Error::Upstream`; gateway maps it to client 502 and applies **`provider-error: 60s`** cooldown — too short for IP risk flags.
- Sentinel hard blocks (`SentinelBlocked`, HTTP 403 on `/sentinel/*`) and incomplete sentinel handshakes can produce the same “unusual activity” copy; both must trigger long cooldown, not 60s retries.
- **Already implemented** (baseline, unchanged in beta.13 except pacing): DPL/script-src cache (**1h**), access-token cache (**5min**), stable `OAI-Device-Id` derived from session cookie, two-stage sentinel (prepare → chat-requirements), Cloudflare cookies (`cf_clearance`, `__cf_bm`, `_cfuvid`) kept in the session jar.
- **Current pacing** (`provider-limits.yaml`, tier `plus-single-session`): **12 rpm**, **3s** min interval, **2** concurrent — tuned for throughput, not web-session risk; each paced “start” still fans out to **~5–8 upstream HTTP calls** (token, sentinel ×2, conversation; warmup ×3 on cache miss).
- Release version for this change: **`0.3.0-beta.13`**.

## Goals / Non-Goals

**Goals:**

- Skip redundant warmup GETs within 60s for the same session identity.
- Invalidate per-session caches when auth fails or upstream signals abuse, so the next attempt does not reuse stale tokens/warmup state.
- When upstream signals **abuse/risk block**, cool down `chatgpt-web` for **hours**, not seconds, so autodefault failover stays fast without hammering ChatGPT.
- Classify abuse from response **body text** (no live API in CI).
- **Reduce chatgpt-web pacing** to a single-session, browser-like profile (see decision §9).
- Regression tests for session-token rotation (unchunked ↔ chunked) preserving CF cookies.
- Tests proving cache behavior, cooldown durations, and updated pacing catalog values.

**Non-Goals:**

- Residential proxy, egress rotation, or multi-account session pool (separate change / ops).
- Per-request fingerprint rotation or sub-second request jitter (possible follow-up if pacing alone is insufficient).
- New CLI commands or login flow changes.
- Detecting abuse for non-`chatgpt-web` providers beyond generic body match + default `abuse-block` fallback.

## Decisions

### 1. Warmup cache location and key

Implement in `crates/chatgpt-web/src/session/warmup.rs` (extract `cache.rs` if file grows).

| Parameter | Value | Rationale |
|-----------|-------|-----------|
| Cache key | `cookie_key(cookie) + ":" + last 8 chars of access_token` | Stable per session; invalidates on token rotation |
| TTL | 60s | Covers burst traffic without stale “warm” state for too long |
| Max entries | 200, evict oldest insertion | Bounded memory in long-running gateway processes |
| Scope | Process-global `LazyLock<Mutex<...>>` | Same pattern as `TOKEN_CACHE` in `session/exchange.rs` |

When cache hit: `run_session_warmup` returns immediately without HTTP.

### 2. Cache invalidation on block

On **401/403** from session exchange, sentinel, or conversation:

- Call existing `invalidate_token_cache(cookie)`.
- Call new `invalidate_warmup_cache(cookie, access_token)` (or clear by cache key).

Rationale: a risk-blocked or expired session must not keep serving cached access tokens or skip warmup on the next gateway retry.

### 3. New cooldown tier: `abuse-block`

Extend `RouterCooldownConfig` and `ProviderCooldownOverrides` with optional field **`abuse_block`** (YAML: `abuse-block`).

| Scope | Default |
|-------|---------|
| `cooldown-defaults` | **2h** (conservative generic fallback) |
| `chatgpt-web` override | **4h** |

Rationale: OpenAI risk flags commonly last hours; 4h avoids retry storms while not blocking forever. Operators can override in YAML later.

### 4. Abuse classification

Add `looks_like_abuse_block(body: Option<&[u8]>) -> bool` in `ai-gateway/src/router/retry_after/` (new `abuse.rs` or extend `classify.rs`).

Match (case-insensitive) on concatenated body text:

- `unusual activity`
- `detected unusual`
- `try again later` **only when paired with** `unusual` or `detected` (avoid over-matching generic 503 copy)
- `sentinel` + `blocked` (executor/dispatcher error messages for hard sentinel failures)

### 5. `cooldown_for_response` behavior

After existing 429 and 401/403/auth branches:

1. For **502 Bad Gateway** (and **503** when body matches): if `looks_like_abuse_block(body)`, return **`config.abuse_block + retry_after_buffer`**.
2. Else fall through to existing **`provider_error`** path.

Implementation note: today 502 does not buffer body — must collect body once (same pattern as 429 branch) when status is 502/503 before choosing cooldown.

Auth errors (`401`/`403`) keep **`auth-error: 30m`** for `chatgpt-web` unless the body explicitly contains unusual-activity copy (then prefer `abuse-block`).

### 6. Session cookie rotation (regression only)

`merge_refreshed_cookie` already drops all `__Secure-next-auth.session-token*` family members before appending refreshed chunks. Add tests for:

- unchunked → chunked rotation (old single token must not remain alongside `.0`/`.1`)
- refreshed blob still includes `cf_clearance` when present in the original DevTools paste

Dropping CF cookies on rotation re-triggers Cloudflare challenges and can cascade into abuse flags.

### 7. Reduce embedded pacing (chatgpt-web)

Today (`plus-single-session` in `provider-limits.yaml`):

| Knob | Current | **New** | Why |
|------|---------|---------|-----|
| `rpm` | 12 | **4** | Cap at ~4 chat completions/min per session — closer to active human use; 12/min on DC egress reads as automation |
| `concurrent` | 2 | **1** | One in-flight completion (single browser tab); parallel sentinel+conv chains from two slots double burst traffic |
| `min-interval-ms` | 3000 | **12000** | At least **12s** between paced starts; with `rpm: 4` the rolling window also enforces ~15s average spacing |

**Upstream load (order of magnitude):**

| Profile | Completions/min | Approx upstream HTTP/min (with warmup cache) |
|---------|-----------------|-----------------------------------------------|
| Current (12 rpm, 2 concurrent, 3s) | up to 12 | ~48–60+ |
| **New (4 rpm, 1 concurrent, 12s)** | up to 4 | ~16–20 |

Warmup cache and DPL/token caches lower the multiplier; pacing lowers the **completion rate** — both are required.

Optional (same PR if trivial): raise `cooldown.rate-limit` **120s → 180s** after HTTP 429 so a hit session rests longer before retry. Not a substitute for lower RPM.

Catalog tests in `provider_limits.rs` and `pacing/limits.rs` must assert the new numbers.

### 8. Executor / dispatcher mapping (minimal)

Keep existing HTTP status mapping in `chatgpt_web::executor` and `dispatcher/chatgpt_web.rs`. Stabilization is **router cooldown policy** plus **executor cache hygiene**, not client-visible error shape changes.

Optional logging: `tracing::warn` with `cooldown_kind = "abuse-block"` when classified.

### 9. Tests (must ship with beta.13)

| Test | Location | Asserts |
|------|----------|---------|
| Warmup skipped on cache hit | `crates/chatgpt-web` | Second `execute` within 60s: MockFetch **0** warmup URLs |
| Warmup runs after TTL | `warmup` unit test | After expiry/clear, 3 warmup GETs fire again |
| Cache cleared on 401 | `executor/tests.rs` | After auth failure, next execute performs warmup again even within TTL |
| Cookie rotation | `session/cookie.rs` tests | unchunked→chunked merge; CF cookies preserved |
| Abuse phrase detection | `retry_after/abuse.rs` | Positive/negative cases incl. sentinel-block copy |
| Cooldown duration | `retry_after/mod.rs` | 502 + abuse body → `4h + buffer` for `chatgpt-web` |
| Catalog parse | `provider_limits.rs`, `pacing/limits.rs` | pacing **4 rpm / 1 concurrent / 12s**; `abuse-block == 4h` |
| Regression | existing executor tests | First-call response sequence unchanged |

## Operational playbook (docs)

Document for operators — not enforced in code in beta.13:

1. **Browser sanity check**: if chatgpt.com shows “unusual activity” in a normal browser on the same egress IP, stop gateway retries for hours; cooldown alone is not enough until the IP clears.
2. **Do not hammer**: repeated autodefault failover that re-selects `chatgpt-web` every minute extends blocks; `abuse-block` cooldown is the fix.
3. **Full cookie jar**: import/login must include session token **and** Cloudflare cookies; bare token-only paste fails sentinel/CF more often.
4. **Egress**: datacenter/pod IPs are high-risk; residential or dedicated egress per session reduces false positives (ops concern).
5. **One session per account**: sharing one session file across many replicas on one IP multiplies traffic patterns that look automated.
6. **Recovery window**: expect **1–24h** for light flags; stop all attempts during wait.

## Risks / Trade-offs

- **[Risk] False positive abuse classification** → Mitigation: tight pattern set; unit tests for negatives.
- **[Risk] 4h cooldown hides recoverable transient 502** → Mitigation: only match known abuse copy; generic 502 stays on `provider-error: 60s`.
- **[Risk] Warmup cache hides stale session state** → Mitigation: key includes access token suffix; explicit invalidation on auth/abuse.

## Migration Plan

1. Implement cache + invalidation + abuse cooldown + tests; run scoped `cargo test` / `clippy`.
2. Bump workspace version **`0.3.0-beta.12` → `0.3.0-beta.13`**.
3. Deploy; if ChatGPT Web was risk-blocked, wait for browser recovery before re-enabling.
4. Rollback: redeploy beta.12; shorter cooldown behavior returns.

## Open Questions

- Should we add optional **request jitter** (0–50ms) in pacing gate for burst-sensitive egress? Defer unless beta.13 still sees 429 bursts at low RPM.
- Should `abuse-block` appear in runtime metrics (`cooldown_kind` label)? Defer unless metrics crate already tags cooldown kind.
