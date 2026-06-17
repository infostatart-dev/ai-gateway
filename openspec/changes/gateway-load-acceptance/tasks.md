# Tasks: gateway-load-acceptance

Stage-driven autodefault hardening + catalog infrastructure + emulator + k6 soak.
Priority labels: **P0** (dead hops), **P1** (failover/parsing), **P2** (observability), **INF** (catalog/emulator infra).

---

## P0 — Stop dead hops (stage blockers)

### T1 — LongCat: exclude from autodefault + 24h cooldown

- [x] Done

**Files**: `config/embedded/model-mapping.yaml`, `router/budget_aware/rank.rs`,
`router/budget_aware/health.rs` or credential cooldown on auth/model errors,
`config/embedded/provider-limits.yaml` (longcat `access-denied` cooldown)

- Remove `longcat/LongCat-Flash-Lite` from autodefault aliases until slug confirmed.
- On `Unsupported model` / sustained 400 on model id: 24h slot cooldown, omit from candidates.
- Remove `longcat => 0` rank anomaly.

**Acceptance**: `routing_load` or unit test — longcat never attempted when in cooldown;
stage replay shows 0 longcat attempts.

---

### T2 — Hard payload pre-flight (remove best-effort tail)

- [x] Done

**File**: `router/budget_aware/payload.rs`

- Delete best-effort branch (relax `min_context_tokens` + `keep_largest_effective_window`).
- When no API-key candidate fits: empty list for API providers; web-session providers remain
  if chunk plan fits.
- Extend `routing_load/scenarios/payload_filter.rs` for 158k dossier → openrouter zero attempts.

**Acceptance**: fat json_schema body skips openrouter/groq TPM-limited; deepseek-web receives traffic.

---

### T3 — ChatGPT Web: 45k upload parts

- [x] Done

**Files**: `crates/web-message-budget/src/lib.rs`, `crates/chatgpt-web/src/conversation/body.rs`

- Add `CHATGPT_UPLOAD_PAYLOAD_TOKENS = 45_000`.
- Set in `plan_conversation_turns` (mirror deepseek `completion/plan.rs`).
- Unit test: ~80k est. → `upload_parts > 1`.
- Extend `routing_load/scenarios/chatgpt_last_resort.rs` — no 413 on fat dossier.

**Acceptance**: `chatgpt_web_upload_parts > 1` for 80k payload; zero 413 in last-resort scenario.

---

## P1 — Failover, parsing, daily quota

### T4 — GitHub Models: normalize response content

- [x] Done

**File**: `middleware/mapper/openai_compatible.rs`

- Non-streaming: parse JSON → `normalize_chat_completion` → deserialize.
- Streaming: `normalize_stream_chunk` per chunk.
- Unit test: content array → string, no MapperError.

**Acceptance**: github-models 200 with array content deserializes; no internal ERROR in autodefault.

---

### T5 — Gemini 503: transient sibling rotation

- [x] Done

**Files**: `router/retry_after/mod.rs`, `router/budget_aware/failover_loop.rs`

- 503 overload → `FailoverClass::Transient` (not Overload sibling-skip).
- Keep `QuotaExhausted` sibling-skip for daily quota only.
- Update `credential_failover.rs` tests: 503 tries gemini-free-2; daily quota skips siblings.

**Acceptance**: `gemini_overload_rotates_to_next_sibling` test passes; 8-key utilization under load.

---

### T6 — Daily quota: proactive RPD + long cooldown

- [x] Done

**Files**: `router/pacing/` (T7 dependency), `router/retry_after/body.rs`,
`config/embedded/provider-limits.yaml` (cloudflare, cerebras `quota-exhausted`, `daily-reset-utc-hour`)

- RPD/TPD gate blocks cloudflare/cerebras before dispatch when exhausted.
- Reactive daily quota → cooldown until daily reset using provider `quota-exhausted` override.
- `routing_load/scenarios/failover_quota.rs` — cloudflare zero re-hops after exhaustion.

**Acceptance**: cloudflare/cerebras 0 attempts after daily cap hit in same UTC day.

---

### T7 — Catalog quota pacing: TPM + RPD/TPD gates

- [x] Done

**Files**: `router/pacing/limits.rs`, `gate.rs`, `config/provider_limits.rs`,
`config/embedded/provider-limits.yaml`

- Extend `PacingLimits` with `tpm`, `rpd`, `tpd`.
- Per-scope counters: RPM/TPM minute window; RPD/TPD daily reset at `daily-reset-utc-hour`.
- Unit tests: RPD reject at limit; TPM reject; daily reset.

**Acceptance**: proactive reject before upstream; pairs with T6.

---

### T8 — Cooldown: provider override for QuotaExhausted

- [x] Done

**File**: `router/retry_after/body.rs`

- Pass `catalog.cooldown_for(provider).quota_exhausted` into `resolve_429_base_secs`.
- Unit test: provider `quota-exhausted: 24h` used when no upstream hint.

**Acceptance**: cloudflare daily cap → 24h-class cooldown, not 60s provider-error.

---

### T9 — OpenRouter 402 / budget probe

- [x] Done

**Files**: new `router/budget_probe/`, wire into candidate selection

- Poll `runtime-sources.key-info` per slot.
- Zero credits + paid model → skip pre-dispatch.
- 402 response → refresh snapshot + cooldown + failover to `:free` or next provider.
- Integration test with mock HTTP.

**Acceptance**: zero 402 on autodefault free-first path; paid skipped pre-dispatch.

---

## P2 — Observability

### T10 — ChatGPT Web chunking metrics

- [x] Done

**Files**: `crates/chatgpt-web/src/executor.rs` (return stats),
`dispatcher/chatgpt_web.rs`, `metrics/provider/dispatch.rs`, observability endpoint

- Add `chatgpt_web_turns`, `chatgpt_web_upload_parts` (mirror deepseek-web).
- Expose in provider-stats route summary.
- Test: `tests/provider_observability.rs` or routing_load assert.

**Acceptance**: GET provider-stats shows chatgpt fields after multi-turn dispatch.

---

## INF — Catalog alignment + emulator

### T11 — `provider-limits.yaml`: catalog fields

Add per active provider: `daily-reset-utc-hour`, `expected-ttfb-ms`, `ms-per-token`.
Extend `ProviderLimitConfig` serde.

**Acceptance**: catalog parses; new fields optional.

---

### T12 — Align `rank.rs` with autodefault-routing-priority

- [x] Done

**File**: `router/budget_aware/rank.rs`, `budget_aware/tests.rs`

- Order: opencode → openrouter → github-models → mistral → groq → cerebras →
  cloudflare → gemini → deepseek-web → anthropic → openai → chatgpt-web.
- chatgpt-web last; deepseek-web before chatgpt-web.

**Acceptance**: ordering tests pass.

---

### T13 — Emulator: universal catalog-driven upstream

**Files**: `crates/upstream-emulator/src/config.rs`, `engine.rs`, `limits/`

- Remove `realistic_provider_latencies()` as source of truth.
- Read TTFB from catalog; enforce RPM/TPM/RPD/TPD via same resolve path as gateway.
- Admin force profiles: 429-rpm, 429-quota, 503-overload, 402, 400-context.
- No separate emulator limit config file.

**Acceptance**: `/_admin/state` limits match catalog; TTFB changes with YAML edit only.

---

### T14 — Verify failover scope (regression)

- [x] Done (503 rotation test updated)

**File**: `router/budget_aware/failover_loop.rs`, existing tests

- Confirm QuotaExhausted skip predicate unchanged.
- Add cross-provider openrouter → groq test if missing.

**Acceptance**: existing gemini quota tests pass + new 503 rotation test from T5.

---

## VERIFY — Load verification (item 22)

### T15 — routing_load scenario coverage for stage fixes

- [x] Done (10/10 including cloudflare_daily_pacing_gate)

**Files**: `routing_load/scenarios/`

- payload_filter: 158k → no openrouter 400.
- chatgpt_last_resort: fat dossier, upload_parts > 1, no 413.
- failover_quota: cloudflare/cerebras daily cap.
- gemini 503 sibling rotation (new scenario).

**Acceptance**: `cargo test --test routing_load --features testing` green.

---

### T16 — Emulated stack verification

- Run harness scenarios against `mise dev:emulated`.
- Document in `DEVELOPMENT.md`.

**Acceptance**: HTTP-level routing_load passes against emulator.

---

### T17 — k6 autodefault soak + CI hook

**Files**: `benchmarks/suite/routing-autodefault.js`, CI workflow (optional nightly)

- k6 script: sustained autodefault load, poll `GET /v1/observability/provider-stats`.
- Assert success rate band, zero longcat attempts, cloudflare zero post-quota.

**Acceptance**: script runs locally; CI job defined (nightly or manual workflow_dispatch).

---

## Suggested implementation order

```
P0: T1 → T2 → T3          (same PR — immediate stage relief)
P1: T4, T5, T7+T6+T8, T9   (failover + pacing — can split 2 PRs)
P2: T10
INF: T11–T14               (parallel with P1 where independent)
VERIFY: T15–T17            (after P0 lands; T17 last)
```

## Stage re-validation checklist

After deploy to stage with dossier-agent load:

- [ ] longcat attempts = 0
- [ ] openrouter 400 context errors = 0
- [ ] cloudflare/cerebras attempts = 0 after first daily quota hit
- [ ] gemini success rate > 20% (8-key rotation)
- [ ] chatgpt-web 413 = 0 on fat dossier
- [ ] github-models deserialize errors = 0
- [ ] overall success rate > 50%
