## Why

Autodefault on stage (`0.3.0-beta.21`) achieved **17.5% success** because the router sends
requests to providers whose outcome is **knowable before HTTP**: dead model slugs, context
overflow, exhausted daily quotas, wrong failover class on 503, missing response normalize,
zero-credit paid routes.

**Goal of this change:** make the **production gateway** stop burning budget on avoidable
failures. The upstream emulator is a **verification vehicle** (same catalog YAML, no live
keys) — not the product deliverable.

## What Changes — gateway first (P0 → P2)

| # | Fix | Capability |
|---|-----|------------|
| 1 | Exclude access-denied providers + 24h slot cooldown | `autodefault-hardening` |
| 2 | Hard payload pre-flight — no best-effort overflow tail | `autodefault-hardening` |
| 3 | ChatGPT Web 45k upload parts (parity with DeepSeek) | `autodefault-hardening` |
| 4 | GitHub Models response normalize before deserialize | `autodefault-hardening` |
| 5 | Gemini 503 → sibling rotation; daily quota → skip siblings | `autodefault-hardening` + `autodefault-routing-priority` |
| 6 | Proactive RPM/TPM/RPD/TPD pacing per credential scope | `catalog-quota-pacing` → **[per-model-quota-domain](../per-model-quota-domain/)** |
| 7 | Cooldown per provider override + per slot state | `provider-cooldown-policy` |
| 8 | OpenRouter dual gate (credits + catalog quotas) | `credential-budget-availability` → **[per-model-quota-domain](../per-model-quota-domain/)** |
| 9 | ChatGPT chunking metrics in provider-stats | `routing-observability` |
| 10 | Provider priority order aligned with living spec | `autodefault-routing-priority` |

## Verification (means, not goals)

- In-process: `routing_load` scenarios (existing `routing-load-verification` framework).
- HTTP-level: catalog-driven emulator when `mise dev:emulated` (T13, T16).
- Optional: k6 soak polling provider-stats (T17).

## Capabilities

### New

- `autodefault-hardening` — P0–P2 routing guardrails (items 1–5, 4).
- `catalog-quota-pacing` — proactive multi-dimension pacing (item 6).
- `provider-cooldown-policy` — catalog cooldown stack (item 7).
- `credential-budget-availability` — runtime budget probe (item 8).

### Modified

- `autodefault-routing-priority` — rank order + failover scope (items 5, 10).
- `routing-observability` — ChatGPT chunk fields (item 9).
- `upstream-provider-emulator` — read same catalog as gateway; **verification only**.
- `routing-load-verification` — delta: stage-gap scenarios prove items 1–5.

**Deferred to [per-model-quota-domain](../per-model-quota-domain/):** items 6–8 (per-model pacing,
OpenRouter 402/429 domain, budget probe integration). Implement there first; return here only for
stage soak (T17) after beta.4 lands.

## Non-Goals

- Emulator as a standalone product or separate latency config files.
- External load scripts, acceptance markdown, `collect-stats.sh`.
- Mistral 401 (ops credential rotation).
- Distributed quota across gateway replicas.
