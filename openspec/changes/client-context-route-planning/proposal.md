## Why

Stage beta.4 (`0.4.2.beta.4`) shows **39% inbound-request failover** and **4.5 upstream attempts per
invoker call** because autodefault walks a long candidate list by trial-and-error. Gateway drivers
already send `X-Agent-Name` and optional `Helicone-Session-Id` (work-unit id), but the gateway
**ignores both** — three parallel invoker work units look identical and collide on the same dead
Gemini keys. Cooldown uses YAML fallbacks instead of pacing windows; ranking ignores per-model
cooldown; provider-stats omits configured-but-idle credentials.

**Client requirement (stability):** when fast free models are exhausted, the gateway MUST deliver a
**stable answer** by escalating **up** the per-model ladder (capacity → stability band, e.g.
`gemini-2.5-flash-lite`) on the **same credential** before jumping to another provider. It MUST
**never downgrade** below the routing intent floor or pick a **smaller/faster** model on another
provider when stability is the goal.

**Goal:** replace blind failover with **caller-aware, quota-aware route planning** on free-tier
pools: read invoker context, score credential health from runtime stats + embedded catalog pacing,
build a **short hop chain** per request, spread parallel work units across live keys, remember
successful routes per work unit, and **escalate up** when fast band is exhausted.

## What Changes

### Layer 1 — Immediate routing fix

1. **Caller request context** — middleware extracts `X-Agent-Name`, `Helicone-Session-Id`, and
   `X-Work-Unit-Id` (preferred when present); attaches `CallerRequestContext` to request extensions;
   echoes work-unit id in route trace / optional response header.
2. **Credential health registry** — rolling success rate and last-error class per
   `(provider, credential_id)` from `ProviderRuntimeRegistry`; **circuit-open** credentials with
   sustained failure; integrate model-level and slot-level cooldown into ranking; cooldown duration
   = `max(upstream Retry-After, pacing gate next-wait)` not YAML constant alone.
3. **Route chain planner** — before failover walk, `plan_route_chain()` filters dead credentials and
   zero-success providers, scores viable `(provider, credential, model)` tuples using health +
   pacing headroom + intent/ladder fit, returns **ordered short chain** (default max 7 hops);
   failover loop walks **plan only**, rebuilds plan on exhaustion.
4. **Caller-aware key spread** — for same `(provider, model)` pool, prefer credential via stable hash
   of `(agent_name, work_unit_id)` among **healthy** slots only (replaces blind round-robin for
   planning input; preserves fairness across work units).
5. **Free-tier catalog integration** — planner consumes embedded `provider-limits.yaml` (per-model
   RPM/RPD/TPM) and `provider-ladders.yaml` (fast → capacity → stability bands) when building hops;
   saturated models excluded at plan time via pacing snapshot, not only after HTTP 429.
6. **Stability escalation (client-ordered)** — when fast-thinking / fast band exhausted on a slot,
   planner appends **capacity** then **stability** ladder models on the **same credential** before
   cross-provider hop; never below `autodefault-intent-routing` floor; never pick a deprioritized
   free model (e.g. openrouter nemotron) when a stability-band model on Gemini still has headroom.

### Layer 2 — Work-unit route memory and live quota

7. **Work-unit route memory** — in-process `WorkUnitRouteMemory` keyed by
   `(agent_name, work_unit_id)` stores last successful `(credential, model)` binding with TTL;
   subsequent calls from the same work unit **prefer** the remembered binding when health and quota
   snapshot still allow it; invalidate on failoverable failure or circuit-open.
8. **Quota headroom scheduling** — at plan time, `QuotaSnapshot::capture()` reads pacing gates +
   health for all free-tier candidates; planner scores by **current** headroom (not post-failure
   cooldown alone); parallel work units avoid assigning the same saturated credential when healthier
   alternatives exist; saturated models skipped without HTTP attempt.
9. **Provider-stats inventory** — snapshot merges **configured** credentials (zero attempts) with
   runtime rows; optional `agent_name` label on attempts when caller context present.
10. **routing_load scenarios** — architectural tests proving layer 1 + layer 2 on concrete free-tier
    use cases (3 work units, circuit-open, hop cap, stability-up, route memory sticky/invalidate,
    quota collision avoidance, catalog pacing skip).

## Capabilities

### New Capabilities

- `caller-request-context`: Parse and propagate invoker + work-unit identity from inbound headers.
- `credential-health-routing`: Runtime health scoring, circuit-open, dynamic cooldown, ranking fixes.
- `route-chain-planning`: Short planned hop chain per request; caller-aware credential spread.
- `work-unit-route-memory`: Sticky last-success route binding per work unit with invalidation.
- `quota-headroom-scheduling`: Live pacing snapshot at plan time; proactive skip of saturated models.

### Modified Capabilities

- `autodefault-routing-priority`: Selection uses planned chain; dead providers deprioritized/excluded.
- `autodefault-intent-routing`: Stability escalation within plan before cross-provider; floor unchanged;
  client stability order (up ladder, not down).
- `routing-observability`: Configured credential inventory in provider-stats; invoker dimension;
  route memory hit/miss in trace.
- `routing-load-verification`: Expanded scenario catalog for caller-context, planning, memory, quota.

## Impact

| Area | Files / systems |
|------|-----------------|
| Middleware | New `caller_context` layer; `types/extensions.rs` |
| Router | `budget_aware/selection.rs`, `sort.rs`, `cooldown.rs`, new `plan/` module, `memory/` module, `failover_loop.rs` |
| Pacing | `PacingGate::peek_next_wait` (read-only); `QuotaSnapshot` in `plan/snapshot.rs` |
| Config | Consumes embedded `provider-limits.yaml`, `provider-ladders.yaml` (no operator YAML break) |
| Metrics | `metrics/provider/runtime.rs`, attempt recorder attrs, trace `route_memory_hit` |
| Crates | `moka` (route memory); keep `governor`/`backon`; no `tower-resilience`/`api-key-pool` |
| Invoker drivers (follow-up) | Gateway driver: pass `session_id` as work-unit id on structured calls — documented, not in this change |
| Tests | `routing_load/scenarios/*`, unit tests in `budget_aware/plan/`, `budget_aware/memory/` |
| CI | `cargo test --test routing_load`, incremental clippy |

## Non-Goals

- Distributed health or route memory across gateway replicas (single-pod in-memory v1).
- Replacing `per-model-quota-domain` pacing work — this change **consumes** existing gates and catalog.
- ChatGPT Web load-sharing (remains last-resort; plan may include only when free band empty).
- Invoker driver changes in this repo (document HTTP header contract only).
- Invoker-side concurrency throttling (document SHOULD; not enforced by gateway).
- **LLM-as-operational-router** — per-request LLM/embedding for credential or quota selection
  (hard constraint D15); gateway routing is deterministic over system state (D16).
- Replay tooling / offline re-simulator — v1 emits `ReplayRecord` log contract only (D19).

## Summary

OpenSpec change **`client-context-route-planning`** turns autodefault from reactive
failover-walk into a **bounded control-plane optimizer** for free-tier multi-key routing.

**Problem:** stage beta.4 — 39% failover, 4.5 attempts/request, 68% 429; dead Gemini keys
still probed; 3 parallel invoker work units collide; gateway ignores caller headers.

**Solution:** Plan-Then-Walk — `QuotaSnapshot` + `FEASIBLE(c)` + `score(c)` → plan ≤7 hops →
bounded walk (+1 replan, ≤14 attempts absolute). Layers: caller context, health/circuit,
quota headroom, route memory (moka), stability ladder UP, 10 routing_load scenarios.

**Architecture:** Gateway = control plane (state, <30ms). Invoker = decision plane (semantics).
D15–D19 hard constraints: no LLM in hot path, contract-only intent, replayable decisions,
stability bounds. Bandit v2 may tune weights only.

**Status:** Spec complete, validate strict OK. Implementation via `/opsx:apply` + `tasks.md`.
