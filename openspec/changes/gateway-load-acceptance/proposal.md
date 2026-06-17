## Why

Stage snapshot (`llm-gateway:0.3.0-beta.21`, ~54 min, 321 upstream attempts, **17.5% success**)
shows autodefault burning budget on avoidable failures instead of routing to working providers:

| Provider | Att | OK | % | Symptom |
|----------|-----|----|---|---------|
| longcat | 64 | 0 | 0% | `Unsupported model (LongCat-Flash-Lite)` ×40, 429 ×24 |
| gemini (8 slots) | 60 | 4 | 7% | 429 RPM + 503 overload — siblings skipped wholesale |
| openrouter | 51 | 12 | 24% | 400 context overflow (158k > 131k), 402 insufficient credits |
| cloudflare | 30 | 0 | 0% | daily cap re-hit every request |
| cerebras | 28 | 0 | 0% | token quota re-hit every request |
| mistral | 62 | 17 | 27% | mixed 400/401 |
| deepseek-web | 16 | 14 | 88% | healthy |
| chatgpt-web | 3 | 2 | 67% | 413 on ~80k single-turn fat dossier |
| groq | 7 | 7 | 100% | healthy |

Root causes cluster into five gaps:

1. **Catalog not enforced before dispatch** — RPM-only pacing; RPD/TPD and credit state discovered only after upstream 429/402/400.
2. **Payload filter fail-open tail** — when no candidate fits context, router relaxes requirements and sends fat dossiers to providers that will 400.
3. **Web-session chunking asymmetry** — DeepSeek uses 45k upload parts; ChatGPT still defaults to 90k → 413 on per-message limits.
4. **Failover class mismatch** — 503 overload skips all free siblings like daily quota; gemini 8-key pool under-utilized.
5. **Emulator drift** — hardcoded TTFB/latency in Rust; not a faithful universal upstream for autodefault verification.

This change closes the stage gaps **and** makes the catalog the single source of truth for
quotas, cooldown, budget probes, expected TTFB, and emulator behaviour — verified by the
existing `routing_load` harness plus optional k6 soak (routing-load-verification §6.1).

## What Changes

### P0 — stop dead hops immediately

- **LongCat governance**: remove from autodefault candidate list until API access restored;
  apply 24h credential cooldown on `Unsupported model` / auth failures; fix or retire stale
  `LongCat-Flash-Lite` slug when access returns.
- **Hard payload pre-flight**: when estimated tokens exceed every candidate's effective window,
  skip all API-key providers — no best-effort fallback that causes openrouter 400.
- **ChatGPT Web conservative chunking**: `upload_part_token_cap = 45_000` (same as DeepSeek);
  fat dossier → multi-turn upload, zero 413 on last-resort path.

### P1 — smarter failover and parsing

- **GitHub Models response normalize**: apply `openai_chat_response::normalize_chat_completion`
  on upstream JSON before deserialize (content array → string).
- **Gemini overload policy**: 429 RPM → transient, try next sibling slot; 503 overload →
  transient sibling rotation (not quota-style skip-all); daily quota → skip siblings + long cooldown.
- **Daily quota proactive gate**: cloudflare/cerebras RPD/TPD exhausted → pacing reject + cooldown
  until catalog daily reset — zero re-hops until next day.
- **OpenRouter 402 guard**: budget probe blocks paid path when `limit-remaining = 0`; failover
  to `:free` slug or next provider without upstream 402.

### P2 — observability

- **ChatGPT Web trace parity**: expose `chatgpt_web_turns` / `chatgpt_web_upload_parts` in
  provider-stats and route trace (same fields as deepseek-web today).

### Infrastructure (from prior scope, retained)

- **`catalog-quota-pacing`**: RPM + TPM + RPD/TPD per credential scope.
- **`provider-cooldown-policy`**: catalog overrides + upstream hints; per-slot state.
- **`credential-budget-availability`**: `CredentialBudgetProbe` over `runtime-sources`.
- **`autodefault-routing-priority`**: align `rank.rs` with living spec; longcat out of band.
- **`upstream-provider-emulator`**: universal YAML-driven stub — limits, TTFB, capabilities,
  failure profiles — no hardcoded Rust latency tables.

### Verification (routing-load-verification task 6.1 / item 22)

- k6 autodefault soak script polling provider-stats.
- Optional CI or nightly job.
- Asserts: success rate band, zero attempts to access-denied providers post-cooldown,
  zero daily-quota re-hops, multi-slot rotation under load.

## Capabilities

### New Capabilities

- `catalog-quota-pacing` — proactive multi-dimension pacing.
- `provider-cooldown-policy` — catalog-driven cooldown per provider + slot.
- `credential-budget-availability` — runtime budget probe.
- `autodefault-hardening` — autodefault routing guardrails (payload, chunking, failover, credits).

### Modified Capabilities

- `autodefault-routing-priority` — code alignment + failover class split (503 vs quota).
- `upstream-provider-emulator` — universal catalog-parameterized upstream.
- `routing-observability` — ChatGPT Web chunking fields in provider-stats.
- `routing-load-verification` — stage-gap scenarios proving gateway-load-acceptance guardrails.

## Impact

- `router/budget_aware/payload.rs` — remove best-effort oversized tail.
- `router/pacing/`, `router/retry_after/`, `router/budget_aware/failover_loop.rs`.
- `crates/web-message-budget/`, `crates/chatgpt-web/` — ChatGPT upload cap.
- `middleware/mapper/openai_compatible.rs` — GitHub Models response normalize.
- `metrics/provider/`, `dispatcher/chatgpt_web.rs` — observability parity.
- `config/embedded/` — longcat cooldown, provider limits, model-mapping.
- `crates/upstream-emulator/` — catalog-only configuration.
- `benchmarks/suite/routing-autodefault.js` — k6 soak (optional CI).

## Non-Goals

- Fixing mistral 401 (credential rotation — ops, not router code).
- External acceptance markdown or `collect-stats.sh`.
- Cross-instance distributed quota counters (in-process + reactive 429 remains acceptable).
