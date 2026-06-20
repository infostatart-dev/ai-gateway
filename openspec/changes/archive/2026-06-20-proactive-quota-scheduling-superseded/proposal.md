## Why

> **Superseded by [`hierarchical-quota-admission`](../archive/2026-06-18-hierarchical-quota-admission/proposal.md)** (shipped **0.5.5**).
> Scheduler/oracle framing was incorrect. **Do not implement this change.**

## Status

**Archived as superseded** — living specs and code live under `quota-admission-control` and
`archive/2026-06-18-hierarchical-quota-admission/`. Delta specs in this folder are historical only.

## Historical context

Release **0.5.x** shipped plan-then-walk routing with `QuotaSnapshot` and ladder escalation, but
production still showed **~20% failover** and many upstream **429** on only two of eight Gemini slots.
The planner treated `peek_next_wait > 0` as «почти готово» (sleep ≤3s and probe HTTP), local pacing
counters drift from upstream reality after 429, and the snapshot is captured once per inbound request
while ladder hops consume quota without re-peek. Operators expect: **per (слот, модель) знать «до
времени T нельзя»**, spread по здоровым слотам, и **повторный 429 на ту же пару = ошибка шлюза**.

## What Changes

1. **Quota oracle (callability gate)** — единый ответ «можно ли звонить сейчас» для
   `(credential_id, model_slug)` из pacing, daily RPD, model/slot cooldown и upstream reset headers;
   `next_wait > 0` ⇒ **zero headroom**, без sleep-probe и без HTTP.
2. **Post-429 pacing sync** — после `QuotaExhausted` / RPM 429 шлюз записывает в local pacing
   «занято до T» (reset header или классификатор), чтобы следующий план не бил в ту же пару.
3. **Repeat-429 guard** — если upstream 429 приходит на `(slot, model)`, уже в cooldown/oracle
   «blocked until T», шлюз логирует `repeat_429_violation`, инкрементирует метрику, не продлевает
   cooldown бесконечно; тесты трактуют это как fail.
4. **Hop-time re-peek** — перед каждым planned hop (и при replan) переснимать oracle для
   оставшихся кандидатов; пропускать hops с `headroom_score == 0` без attempt counter.
5. **Ladder headroom filter** — intra-slot ladder включает только модели с положительным oracle
   на момент плана и на момент hop.
6. **Observability** — `next_available_at` / `blocked_reason` per credential row in
   provider-stats routing block; `repeat_429_violations` counter.
7. **Tests & scenarios** — unit oracle matrix, `routing_load` сценарии: zero-repeat-429,
   N parallel work units → N distinct headroom slots, hop re-peek after first 429.

**Explicit non-goals:**

- Redis-backed oracle (follow-up `route-memory-redis` / distributed pacing).
- Изменение invoker (Graphiti) — только шлюз.
- KPI «upstream 429 = 0 при полном исчерпании пула» — цель **≈0 repeat** на известно-заблокированных парах, не ноль первичных 429 от upstream.

Ships in **`0.5.5`** (routing line, not 0.4.x).

## Capabilities

### New Capabilities

- `quota-oracle-gate`: Unified callability oracle per `(credential, model)` — combines pacing peek,
  daily headroom, cooldown timers, upstream reset; hard skip when `next_wait > 0`; repeat-429 guard.

### Modified Capabilities

- `quota-headroom-scheduling`: Zero-wait threshold (`next_wait > 0` ⇒ score 0); hop-time re-peek;
  post-429 pacing sync into snapshot inputs.
- `route-chain-planning`: Ladder hops filtered by oracle at plan and hop time; replan uses fresh
  oracle not stale snapshot.
- `per-model-exhaustion-scopes`: 429 `QuotaExhausted` / RPM applies pacing block until reset T;
  feeds oracle.
- `routing-load-verification`: Scenarios `zero_repeat_429`, `parallel_headroom_spread`,
  `hop_repeek_after_429`.
- `routing-observability`: `next_available_at`, `blocked_reason`, `repeat_429_violations` in
  provider-stats / route trace.

## Impact

- `router/budget_aware/plan/snapshot.rs` — oracle integration, zero-wait rule
- `router/budget_aware/cooldown.rs` — remove sleep-probe for planned hops; skip only
- `router/budget_aware/plan/build.rs`, `dispatch.rs` — hop re-peek, ladder filter
- `router/pacing/gate.rs` — `apply_upstream_block_until`, sync from 429
- `router/retry_after/` — reset T propagation to pacing
- `metrics/provider/` — repeat-429 violation, next_available_at
- `ai-gateway/tests/budget_aware_snapshot.rs`, `tests/rl/scenarios/`
- `CHANGELOG.md`, `Cargo.toml` → **0.5.5**

## Related Changes

| Change | Relationship |
|--------|----------------|
| `per-model-quota-domain` (0.4.2-beta.4) | Shipped per-model scope + OpenRouter; this hardens scheduling |
| `routing-ops-hardening` (0.5.1) | Observability naming; extends with oracle fields |
| `client-context-route-planning` (archived) | Original plan-then-walk; this closes scheduling gaps |
