## Context

**Today (beta.14–18):**

| Area | `chatgpt-web` | `deepseek-web` |
|------|---------------|----------------|
| JSON schema | Prompt injection + validate + retry | Not supported; capability `false` |
| Long context | `web-message-budget::plan_web_chunks` | Flat `messages_to_prompt`, `history_window: 0` |
| Autodefault mapper | `ChatGptWebConverter` registered | **Missing** → `Converter not present` |
| Multi-turn in one request | Conversation chain (`conversation_id`, `parent_message_id`) | Single session, one completion, delete session |
| Structured gate | `structured_output_valid` uses `chatgpt_web::schema` | N/A |

DeepSeek upstream accepts a single `prompt` string per completion (not OpenAI
message array). Each completion requires **token exchange → session create →
PoW → SSE completion → session delete**. Chunking reuses the **same
`chat_session_id`** across upload turns inside one gateway request.

Existing shared crate: `web-message-budget` (parse, estimate, chunk plan,
upload headers). Schema helpers live in `chatgpt-web/src/schema.rs` today.

### DeepSeek limits (researched 2026-06-16)

Sources differ by surface — **web path is not documented to 1M**:

| Surface | Published limit | Source |
|---------|-----------------|--------|
| **DeepSeek API V4** (`deepseek-v4-flash` / `-pro`) | **1M** context, **384K** max output; native JSON output | [api-docs.deepseek.com/pricing](https://api-docs.deepseek.com/quick_start/pricing) |
| **Legacy API aliases** `deepseek-chat` / `deepseek-reasoner` | Map to V4 Flash non-thinking / thinking until 2026-07-24 | Same doc footnote |
| **Older API reports** | **128K** context (input+output shared) | Community/API 400 errors (e.g. Zed #45456) |
| **chat.deepseek.com web UI** | **No per-message character cap** published; cumulative session token budget; banner *"Length Limit Reached, Start a New Chat"* | Third-party guides + operator reports; **not** in official web API docs |
| **Gateway embedded catalog today** | `context-window: 65536` for both web models | `providers.yaml` — **heuristic, never live-probed** |

**Implication:** We MUST NOT assume 1M for `deepseek-web` until a live probe
against `chat.deepseek.com` succeeds. API docs describe the **paid API**, not
our reverse-engineered browser completion endpoint.

**Working default for chunk planner (beta.19 ship):** **`128_000`** input budget
— aligns with historical DeepSeek chat/reasoner API window and is 2× today's
embedded 65k guess. **`deepseek probe --context-limit`** (new) binary-searches
max single-prompt size and writes result to operator notes / future catalog
override.

## Goals / Non-Goals

**Goals:**

- Parity with `chatgpt-web` for **strict JSON schema** and **context upload
  chunking** on the operator-facing OpenAI API surface.
- **Autodefault-safe:** mapper registration + `supports_json_schema: true` for
  **`deepseek-chat` and `deepseek-reasoner`** (same code path; validation on
  `content`, not `reasoning_content`).
- **No silent truncation**; oversized payloads split into upload parts.
- **PoW answer cache** for multi-turn uploads (same gateway request) — required,
  not optional.
- **Live context probe** CLI before raising catalog beyond 128k.
- CI without live DeepSeek for unit/mock paths.
- Ship in **`0.3.0-beta.19`**.

**Non-Goals:**

- Native DeepSeek API (`deepseek` provider) changes.
- Tools / function calling on DeepSeek Web.
- Streaming structured-output validation (same fail-open as other providers).
- Cross-request conversation memory.
- Bypassing pacing for upload parts.

## Decisions

### 1. Shared structured-output module

Extract schema parse/inject/validate from `chatgpt-web` into
`crates/web-structured-output` (or `web-message-budget/structured`).

`chatgpt-web` and `deepseek-web` both depend on it. Gateway structured-output
gate imports from the shared crate.

### 2. Context budget and chunk sizes

**Decision:** `DEEPSEEK_WEB_CONTEXT_TOKENS = 128_000` in chunk planner and
`providers.yaml` / capability catalog (replace 65_536).

**Decision:** DeepSeek-specific upload part cap **`DEEPSEEK_UPLOAD_PAYLOAD_TOKENS
= 45_000`** (not ChatGPT's 90k) so each part fits comfortably inside 128k
budget with system overhead, schema block, and reserved output.

**Decision:** Add `deepseek probe --context-limit` — escalating prompt sizes
until upstream error; prints recommended `context-window` for operators. Does
not auto-mutate embedded yaml in CI.

**Alternative rejected:** Copy API 1M into web catalog without probe — high risk
of silent upstream failures mid-upload.

### 3. Executor multi-turn loop

Same flow as prior design: one token exchange, one session, loop turns, delete
session at end. Schema on final turn only.

### 4. PoW cache (in scope — was wrongly listed as defer)

**Decision:** In-process PoW cache keyed by `(access_token_prefix, chat_session_id)`
with **TTL 45s**, max **64** entries.

Reuse cached `X-Ds-Pow-Response` for subsequent upload turns **within the same
gateway request** when challenge salt/expiry still valid.

**What we risked by deferring (why it is in scope now):**

| Without PoW cache | Impact |
|-------------------|--------|
| 6 upload parts | 6× SHA3 brute-force solves (~seconds each) |
| + `min-interval-ms: 10000` | 6 turns ≈ **≥60s wall time** minimum |
| DeepSeek abuse heuristics | Burst of PoW+completion from one session reads as automation → **429 / auth cooldown** |
| Operator experience | Fat json_schema dossier feels "broken" vs ChatGPT Web |

**Residual risk with cache:** Stale PoW if upstream rotates challenge mid-request
→ executor catches 4xx, invalidates cache entry, refetches challenge once.

### 5. JSON schema on chat and reasoner (same behavior)

**Decision:** Both `deepseek-chat` and `deepseek-reasoner` ship with
`supports_json_schema: true` in beta.19. Same executor path: inject schema,
validate, retry.

For **reasoner**, structured output validation applies to assistant **`content`**
(the ANSWER SSE fragment), **not** `reasoning_content` (THINK). The gateway
returns both fields in the OpenAI response; only `content` must be valid JSON.

**Decision:** `deepseek probe --structured-output` is an **operator smoke test**
(like existing `deepseek probe --query`). It is **not** a release gate and does
**not** toggle capability flags. Pass/fail is printed to the operator; CI uses
mocks only.

**Alternative rejected:** Disable reasoner json_schema when a manual pre-release
probe fails — that leaves half the catalog broken with no runtime recovery.

### 6. Mapper + autodefault

Register `DeepSeekWebConverter`; `json_schema_rank = 0` for browser provider.

### 7. Pacing and observability

Each turn = one paced start. Trace fields: `deepseek_web_turns`,
`deepseek_web_upload_parts`, `deepseek_web_pow_cache_hits`.

## Risks / Trade-offs

| Risk | Mitigation |
|------|------------|
| 128k default still wrong for web | `probe --context-limit`; operator doc explains API≠web |
| PoW cache stale | Single refetch + retry on 4xx |
| Reasoner emits THINK + JSON in content | Validate `content` only; retries target final ANSWER |
| 45k parts → many turns on 1M-class future | Catalog bump after probe reduces turn count |
| Shared crate extraction breaks ChatGPT | Regression tests before DeepSeek wiring |

## Migration Plan

1. Shared structured-output + ChatGPT migration (no behavior change).
2. PoW cache + multi-turn executor + schema loop.
3. Mapper + capability flags for **both** web models.
4. `probe --context-limit` + optional `probe --structured-output` (diagnostics).
5. **CHANGELOG `[0.3.0-beta.19]`** + bump `Cargo.toml`.
6. Rollback: disable capability flags + mapper; direct path reverts to single-turn.

## Resolved questions (formerly open)

| Question | Resolution |
|----------|------------|
| beta.19 vs beta.18 | **beta.19** — workspace already at beta.18 |
| Reasoner + JSON | **Same as chat** — always enabled; validate `content` only |
| `probe --structured-output` | **Operator smoke test**, not a feature flag |
| PoW cache | **Required in beta.19** |
| Context / part sizes | **128k budget**, **45k upload parts**, `--context-limit` to calibrate |
| CHANGELOG | **Mandatory beta.19 section** — see spec requirement |
