## Context

### Stage evidence (2026-06-17, dossier-agent load)

```
POST /router/autodefault/chat/completions  model=openai/gpt-5-mini
~158k input + 4k output reserved

321 attempts → 56 OK (17.5%)
Top noise (20 min): 400×20, 429×18, 200×16, 503×8
```

Dead-hop pattern: router selects a candidate → upstream returns predictable failure →
failover tries next → repeat. Many failures are **knowable before HTTP**.

### What the code does today

**Payload filter** (`budget_aware/payload.rs`):
- Computes `effective_window` from catalog context + TPM cap + margin.
- When **no** candidate fits `min_context_tokens`, falls through to **best-effort tail**:
  relaxes requirement, picks largest window → openrouter 400 on 158k dossier.

**Web chunking** (`web-message-budget/chunk.rs`):
- DeepSeek: `upload_part_token_cap = 45_000` (`DEEPSEEK_UPLOAD_PAYLOAD_TOKENS`).
- ChatGPT: inherits default `90_000` → single-turn passes budget check but hits ChatGPT
  per-message limit (~80k est.) → 413 `message_length_exceeds_limit`.

**Failover** (`failover_loop.rs`, `retry_after/mod.rs`):
- `429` RPM → `FailoverClass::Transient` → next sibling tried ✓
- `503` overload → `FailoverClass::Overload` → **skip all** same-provider same-rank siblings ✗
- Daily quota → `QuotaExhausted` → skip siblings ✓ but **no proactive RPD gate** → re-hit

**Pacing** (`pacing/gate.rs`):
- RPM + concurrent only. RPD/TPD discovered post-429.

**GitHub Models** (`openai_compatible.rs`):
- Response passthrough without `normalize_chat_completion` → deserialize error on content array.

**Observability** (`metrics/provider/dispatch.rs`):
- `deepseek_web_turns`, `deepseek_web_upload_parts` logged; ChatGPT equivalents absent.

**LongCat** (`rank.rs`, `model-mapping.yaml`):
- Rank 0 (ahead of documented cascade). Model slug stale / no API access → 64 dead hops.

**Emulator** (`upstream-emulator/config.rs`):
- `realistic_provider_latencies()` hardcoded; limits partially catalog-driven but TTFB not.

```
┌─────────────────────────────────────────────────────────────────┐
│              AUTODEFAULT CANDIDATE PIPELINE (today)             │
└─────────────────────────────────────────────────────────────────┘
  Request
    │
    ▼
┌──────────────┐    miss     ┌──────────────────┐
│ Rank + cost  │────────────▶│ Payload filter   │
│ class sort   │             │ (best-effort     │
└──────────────┘             │  tail on miss!)  │
    │                        └────────┬─────────┘
    │                                 │
    ▼                                 ▼
┌──────────────┐             ┌──────────────────┐
│ RPM pacing   │             │ HTTP dispatch    │◀── RPD/402/413
│ gate only    │             │ (many avoidable) │    discovered here
└──────────────┘             └──────────────────┘
```

## Goals / Non-Goals

**Goals**
- Zero upstream calls when outcome is knowable from catalog + payload estimate.
- Stage P0 fixes first: longcat out, hard payload gate, ChatGPT 45k chunks.
- P1: GitHub normalize, gemini 503 policy, daily quota cooldown, OpenRouter 402 guard.
- P2: ChatGPT observability parity.
- Universal catalog-driven upstream emulator for autodefault verification.
- k6 soak script (routing-load-verification §6.1).

**Non-Goals**
- Distributed quota across gateway replicas.
- Mistral credential rotation (ops).
- New external config files for emulator latency.

## Decisions

### D1 — LongCat: exclude + cooldown, not rank-0 probe

LongCat API access is blocked / model slug stale. Until operator restores access:

1. Remove `longcat-default` from autodefault when credential returns `Unsupported model` or
   sustained 401/400 on model id at startup health or first dispatch.
2. Apply **24h credential cooldown** (`abuse-block`-class or dedicated `access-denied` cooldown
   in catalog) so autodefault never spends 64 hops/session rediscovering failure.
3. Remove `longcat/LongCat-Flash-Lite` from `model-mapping.yaml` autodefault aliases until
   new model slug confirmed in `providers.yaml`.
4. `default_provider_budget_rank`: longcat NOT in rank-0 band; when re-enabled, insert within
   free-API band per `curated-free-providers-expansion`.

**Rejected**: keep longcat at rank 0 and rely on failover — proven −64 dead hops/session.

### D2 — Hard payload pre-flight (remove best-effort tail)

When `RequestRequirements.min_context_tokens` is set (fat dossier / json_schema path):

```
IF no candidate fits effective_window:
  RETURN empty candidate list for API-key providers
  LET web-session providers (deepseek-web, chatgpt-web) remain if they fit chunk plan
```

Remove `filter_payload_capable` best-effort branch (lines 55–70: relax + largest window).
Unknown context window (`None`) continues to fail-open per existing D3.

Effective window formula unchanged:
`min(context_window, per_request_token_cap) × payload_margin`.

**Scenario**: 158k dossier → openrouter (131k window) **never attempted**; deepseek-web /
chatgpt-web chunking path receives traffic.

### D3 — ChatGPT Web: 45k upload parts (parity with DeepSeek)

Add `CHATGPT_UPLOAD_PAYLOAD_TOKENS: usize = 45_000` in `web-message-budget`.
Set in `chatgpt-web/conversation/body.rs` `plan_conversation_turns` — same pattern as
`deepseek-web/completion/plan.rs`.

Rationale: ChatGPT upstream enforces per-message size below nominal context window.
90k default passes `input_token_budget` math but fails wire limit → 413.

**Acceptance**: ~80k est. prompt → `upload_parts > 1`, zero 413 on dossier last-resort test.

### D4 — GitHub Models: normalize before deserialize

In `OpenAICompatibleConverter` response path (non-streaming + stream chunks):
call `openai_chat_response::normalize_chat_completion` / `normalize_stream_chunk` on raw
JSON **before** `serde_json::from_value` — same as openrouter/cloudflare mappers.

Do not add provider-specific bindings hacks; reuse shared normalizer.

### D5 — Gemini 503 vs 429: sibling policy split

| Failure | FailoverClass | Sibling behaviour |
|---------|---------------|-------------------|
| 429 RPM / transient | Transient | Try next slot at same rank |
| 503 overload / high demand | **Transient** (changed) | Try next gemini-free slot |
| Daily / quota exhausted | QuotaExhausted | Skip all same-rank siblings |

Implementation:
- `classify_and_cooldown`: 503 with overload body → `FailoverClass::Transient` + short
  `provider-error` cooldown (not sibling-skip class).
- Reserve `Overload` class only for unrecoverable upstream outage (optional: repeated 503 on
  **all** siblings → escalate to cross-provider).
- Emulator: `ForcedProfile::Overload` returns 503 JSON parseable by gateway; routing_load
  scenario verifies sibling rotation.

**Rejected**: 503 → skip all 8 gemini-free keys (7% success under load).

### D6 — Daily quota: proactive RPD + long cooldown

Combine catalog pacing (D7) with cooldown policy:

- **Proactive**: RPD/TPD gate rejects before dispatch when daily counter exhausted.
- **Reactive**: `QuotaExhausted` from body (cloudflare "daily free allocation", cerebras
  `token_quota_exceeded`) → cooldown until `daily-reset-utc-hour` from catalog, using
  provider `quota-exhausted` override (e.g. 24h), not 60s provider-error.

Cloudflare neurons / cerebras TPM daily: treat as **RPD/TPD dimension**, not RPM.

### D7 — Catalog quota pacing (RPM + TPM + RPD/TPD)

Extend `PacingLimits` with `tpm`, `rpd`, `tpd`.
Per credential scope counters:
- RPM/TPM: sliding 60s window.
- RPD/TPD: daily counter reset at `daily-reset-utc-hour`.

Pre-dispatch reject → pacing error with dimension-appropriate retry-after.

### D8 — Credential budget probe (OpenRouter 402)

`CredentialBudgetProbe` from `runtime-sources.key-info`:
- `limit_remaining = 0` + paid model → skip slot pre-dispatch.
- `limit_remaining = 0` + `:free` model → allow under catalog RPM/RPD.
- 402 response → terminal for that slot + refresh probe snapshot.

### D9 — Cooldown stack (unchanged core, quota fix)

1. Upstream Retry-After / JSON hint.
2. Provider `cooldown` override for failure class.
3. Global `cooldown-defaults`.

Fix: `QuotaExhausted` without hint → provider `quota-exhausted` override (not global 1h only).

### D10 — Routing priority alignment

```
opencode → openrouter → github-models → mistral → groq → cerebras →
cloudflare → gemini → deepseek-web → anthropic → openai → chatgpt-web
```

Remove `longcat=0`, `chatgpt-web=0`. chatgpt-web MUST be last.

### D11 — Universal upstream emulator

Single YAML catalog drives **all** emulator behaviour:

```yaml
# provider-limits.yaml (per provider)
expected-ttfb-ms: 320
ms-per-token: 0.05
daily-reset-utc-hour: 0
tiers:
  free:
    limits: { rpm: 20, tpm: 60000, rpd: 50 }
```

Emulator responsibilities:
- Resolve limits via **same** `resolve_limits` path as gateway pacing.
- Enforce RPM/TPM/RPD/TPD before response generation.
- Delay = `expected-ttfb-ms + completion_tokens × ms-per-token` (from catalog).
- Capabilities from `providers.yaml` (422 on unsupported json_schema).
- Failure profiles via `/_admin/force` (429-rpm, 429-quota, 503-overload, 402, 400-context).

No `realistic_provider_latencies()` table. No separate `dev/emulated-load.yaml`.

Verification stack:
```
routing_load tests (in-process) ──▶ emulator HTTP (mise dev:emulated) ──▶ k6 soak (optional)
```

### D12 — ChatGPT observability parity

Extend `WebSessionStats` / provider metrics recorder:
- `chatgpt_web_turns`, `chatgpt_web_upload_parts` on dispatch completion.
- Surface in `GET /v1/observability/provider-stats` and route trace JSON.

Executor must return stats struct (mirror deepseek-web `ExecuteStats`).

## Risks / Trade-offs

| Risk | Mitigation |
|------|------------|
| Hard payload gate rejects all API providers on 158k dossier | deepseek-web + chatgpt-web chunk path remains |
| 45k ChatGPT parts → more turns / latency | acceptable vs 413 hard fail on last resort |
| 503 as Transient may hammer overloaded upstream | per-slot cooldown + pacing RPM still applies |
| In-process RPD resets on restart | reactive QuotaExhausted + emulator tests |
| Rank realignment shifts traffic | routing_load scenarios + stage re-run |

## Migration Plan

1. **P0** (same PR or first): longcat exclude, payload hard gate, ChatGPT 45k chunks.
2. **P1**: github normalize, gemini 503 policy, RPD pacing + daily cooldown, OpenRouter probe.
3. **P2**: ChatGPT observability fields.
4. **Infra**: catalog pacing, cooldown fix, rank alignment, emulator catalog TTFB.
5. **Verify**: `cargo test --test routing_load --features testing`, `mise dev:emulated`, k6 script.
6. **Stage**: redeploy → re-run dossier-agent → target success >50% (dead hops eliminated).
