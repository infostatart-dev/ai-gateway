## Context

**Shipped:** `autodefault-intent-routing`, `gemini-per-model-quota-ladder`,
`autodefault-credential-pools`, `per-model-quota-domain` (pacing per model),
`routing-observability` (provider-stats, route trace).

**Observed on stage beta.4 (2.7h soak):** 1389 invoker requests, 39% with internal
failover, 4.5 upstream attempts/request, 68% HTTP 429. Eight of ten Gemini free keys
~5% success; keys 9–10 ~70%. Cloudflare/GitHub 0% success but still attempted.

**Client requirement:** deliver stable structured answers even when fast free models
are exhausted — escalate **up** (`gemini-3.1-flash-lite` → `gemini-2.5-flash-lite`)
on the **same Gemini project** before cross-provider failover. Never downgrade to a
smaller model on another provider when the intent floor is `fast-thinking`.

**Invoker driver contract:** OpenAI-compatible gateway drivers send `X-Agent-Name`,
`Helicone-Property-Agent`, optional `Helicone-Session-Id`. Typical deployment runs
**3 parallel work units** per invoker process (bounded worker pool) but often omits
work-unit id on structured calls (prompt + Pydantic schema). Gateway reads **none**
of these for routing.

**Code today:**

| Layer | Location | Gap |
|-------|----------|-----|
| Headers | — | No `CallerRequestContext` |
| Candidate order | `selection.rs`, `sort.rs` | Full list; model cooldown ignored in rank |
| Key pick | `credential_balance.rs` | Blind round-robin among equal ranks |
| Cooldown | `retry_after/`, `health.rs` | YAML fallback; not merged with pacing |
| Health | `provider_attempt.rs` | failures++ only; no success rate / circuit |
| Failover | `failover_loop.rs` | Linear walk all candidates |
| Ladder | `ladder_rank.rs`, `provider-ladders.yaml` | Escalation exists in rank; not in short plan |
| Pacing | `pacing/gate.rs` | `next_wait` private; no plan-time snapshot |
| Stats | `metrics/provider/runtime.rs` | Attempt-only rows; no configured inventory |
| Route memory | — | Each call plans from scratch |

## Goals / Non-Goals

**Goals:**

1. Extract **caller identity** (invoker name + work unit) once per inbound router request.
2. Maintain **credential health** from runtime attempt outcomes + cooldown/pacing state.
3. **Plan a short route chain** (≤7 hops) per request instead of walking 80–150 candidates.
4. **Spread parallel work units** across healthy Gemini/OpenRouter slots via stable hash.
5. **Dynamic cooldown** aligned with upstream signals and pacing `peek_next_wait`.
6. **Stability escalation up** within plan (ladder capacity → stability band) before cross-provider.
7. **Remember successful routes** per work unit; invalidate on failure — faster repeat calls.
8. **Quota snapshot at plan time** — skip saturated models before HTTP using catalog + pacing.
9. Prove behavior via **routing_load** scenarios (architectural tests, no live keys).

**Non-Goals:**

- Multi-replica shared health or route memory (in-memory v1; document limitation).
- Invoker driver changes in this repo (HTTP contract only).
- ChatGPT Web as load-sharing peer (last-resort unchanged).
- Replacing model ladder or intent floor semantics.
- Gateway-enforced invoker concurrency cap.

## Architecture — planning pipeline

```text
Inbound request
      │
      ▼
CallerRequestContext (middleware)
      │
      ▼
┌─────────────────────────────────────────────────────────┐
│ QuotaSnapshot::capture()                                │
│   pacing gates (per-model RPM/RPD/TPM from catalog)     │
│   credential health (circuit, success rate)               │
│   model/slot cooldown states                            │
└──────────────────────────┬──────────────────────────────┘
                           │
      ┌────────────────────┼────────────────────┐
      ▼                    ▼                    ▼
 WorkUnitRouteMemory   Candidate filters    Catalog ladders
 lookup(binding)       intent/payload/ladder  provider-limits
      │                    │                    │
      └────────────────────┼────────────────────┘
                           ▼
              RouteChainPlanner::plan()
                           │
                           ▼
              Vec<BudgetCandidate> ≤ 7 hops
                           │
                           ▼
              failover_loop (walk plan only)
                           │
              ┌────────────┴────────────┐
              ▼                         ▼
         success                   failoverable fail
    memory.record(binding)      memory.invalidate(binding)
                                replan once (exclude failed)
```

**Abstraction boundaries (small modules, ~60 lines each):**

| Module | Responsibility |
|--------|----------------|
| `plan/snapshot.rs` | `QuotaSnapshot`, `headroom_score()` |
| `plan/score.rs` | Tuple scoring: health × headroom × ladder × hash |
| `plan/build.rs` | Chain construction: spread → intra-slot ladder → cross-provider |
| `plan/mod.rs` | `plan_route_chain()` orchestration |
| `memory/registry.rs` | `WorkUnitRouteMemory` on `moka` cache (get/put/invalidate) |
| `memory/binding.rs` | `RouteBinding { credential_id, model }` |

## Decisions

### D1 — `CallerRequestContext` in request extensions

New middleware layer on router stack (after auth, before budget-aware dispatch):

```text
work_unit_id = X-Work-Unit-Id
            ?? Helicone-Session-Id
            ?? (none)
agent_name   = X-Agent-Name
            ?? Helicone-Property-Agent (strip prefix)
            ?? "unknown-invoker"
```

Stored as `CallerRequestContext { agent_name, work_unit_id }` in `extensions`.

**Alternative rejected:** Require only `X-Work-Unit-Id` — breaks existing callers that send only `Helicone-Session-Id`.

### D2 — Credential health registry (in-process)

New `CredentialHealthRegistry` beside `ProviderRuntimeRegistry`:

```text
Key: (provider, credential_id)
Fields:
  attempts_window: rolling 5 min (success, rate_limited, other_error)
  circuit_open_until: Option<Instant>
  last_failover_class: Option<FailoverClass>
```

Circuit opens when: ≥5 attempts in window AND success_rate < 10%, OR slot-level
project exhaustion, OR auth error (401). Closes on success or TTL (default 15 min).

**Two-tier breaker taxonomy** (industry: separate RPM transient from quota exhaustion):

| Class | Typical signal | Cooldown / circuit | Planner behavior |
|-------|----------------|--------------------|------------------|
| `RateLimited` | HTTP 429 RPM, short Retry-After | seconds–minutes via pacing `peek_next_wait` | skip hop in plan; model deprioritized |
| `QuotaExhausted` | RPD/TPD zero, daily quota body | hours via `quota-exhausted` + daily gate | slot circuit; exclude credential from plan |
| `AuthError` | HTTP 401 | 5m slot circuit | exclude credential immediately |
| `PersistentFailure` | <10% success over window | 15m circuit | exclude credential from plan |

Maps to existing `classify_and_cooldown` + pacing; planner consumes the merged
effective state via `QuotaSnapshot` and health queries — no duplicate classifier.

**Alternative rejected:** External Redis — ops complexity; v2 if multi-replica pain.

### D3 — Route chain planner module

New `router/budget_aware/plan/`:

```text
plan_route_chain(
  candidates, requirements, intent, caller_ctx,
  health, snapshot: QuotaSnapshot, limits, ladders,
  memory: &WorkUnitRouteMemory,
  max_hops: 7,
) -> Vec<BudgetCandidate>
```

Pipeline:

```
ALL candidates
    │ filter: intent/payload/ladder (existing)
    │ filter: circuit_open credentials
    │ filter: snapshot headroom == 0 (proactive skip)
    │ filter: provider zero-success window
    ▼
SCORE each survivor
    │ cost-class, provider priority, health, ladder band
    │ snapshot.headroom_score(credential, model)
    │ caller hash boost for (provider, model) pool
    │ memory affinity boost if binding still viable
    ▼
BUILD chain:
    0. if memory binding viable → first hop
    1. best preferred-band hop per provider tier (hash spread)
    2. intra-slot ladder steps UP (fast → capacity → stability)
    3. next provider in cost-class order (never below floor)
    ▼
TRUNCATE to max_hops
```

`ordered_candidates()` returns **plan**; `failover_loop` walks plan only. On plan
exhaustion, **replan once** with failed hops excluded.

**Alternative rejected:** Keep full walk + only re-rank — does not cap attempts.

### D4 — Caller-aware credential spread

For pool key `(provider, model)` with multiple healthy credentials:

```text
preferred_idx = stable_hash(agent_name, work_unit_id) % healthy.len()
rotate healthy[preferred_idx..] ++ healthy[..preferred_idx]
```

If `work_unit_id` is `None`, fall back to existing round-robin counter.

When `QuotaSnapshot` shows credential A has `next_wait > 0` and credential B has
`next_wait == 0` for the same model pool, spread prefers B even if hash favored A.

### D5 — Dynamic cooldown merge

On failure classification:

```text
cooldown = max(
  classify_and_cooldown_duration,
  pacing_gate.peek_next_wait(estimated_tokens),
)
```

Ranking uses **effective cooldown** = max(credential_state, model_state).

`max_cooldown_wait` unchanged (3s) for wait-vs-skip during walk.

### D6 — Stability escalation in plan (client-ordered, UP only)

Aligned with `provider-ladders.yaml` gemini free bands:

```text
fast:      gemini-3-flash-preview, gemini-3.5-flash
capacity:  gemini-3.1-flash-lite, gemini-2.5-flash, gemini-2.5-flash-lite
stability: gemini-2.5-flash-lite
```

Planner intra-slot ladder rules:

1. Try fast band models first (when intent allows).
2. When fast band unavailable (cooldown, RPD exhausted, circuit), append **capacity**
   models on **same credential** in ladder order.
3. When capacity exhausted, append **stability** model (`gemini-2.5-flash-lite`).
4. Only then append cross-provider hops within `escalation_ceiling`.

**Forbidden:**

- Selecting openrouter `nemotron` (deprioritized band) while Gemini stability band
  has headroom on any healthy slot.
- Selecting a model with lower `intent_tier` than `floor_tier` on another provider.
- Downgrading from `gemini-3.1-flash-lite` to `gemini-3-flash-preview` on failover.

Existing `gemini_stability_escalates_up` routing_load scenario proves the ladder walk
in failover; this change moves that ordering into **plan construction** so attempts ≤ 7.

### D7 — Observability deltas

- `AttemptRecord` + provider-stats: optional `agent_name` label.
- Snapshot: merge `credentials.yaml` rows with zero attempts (`status: idle`).
- `PendingRouteTrace`: `agent_name`, `work_unit_id`, `planned_hops`, `plan_rebuilds`,
  `route_memory_hit` (bool), `route_memory_invalidated` (bool).

### D8 — Test architecture (`routing_load`)

| Scenario file | Proves | Layer |
|---------------|--------|-------|
| `caller_three_work_units.rs` | 3 work units → distinct healthy keys | 1 |
| `credential_circuit_open.rs` | circuit → zero attempts | 1 |
| `route_plan_max_hops.rs` | attempts ≤ 7 | 1 |
| `stability_escalation_plan.rs` | flash-lite before openrouter | 1 |
| `dynamic_cooldown_skip.rs` | pacing skip without HTTP | 1 |
| `route_memory_sticky_reuse.rs` | 2nd call same work_unit → same binding | 2 |
| `route_memory_invalidate_on_429.rs` | 429 invalidates; 3rd call new key | 2 |
| `quota_parallel_collision.rs` | 3 units, 2 headroom keys → no triple-hit | 2 |
| `stability_never_downgrade.rs` | fast dead → stability up, not nemotron | 1+2 |
| `free_catalog_pacing_skip.rs` | RPD-saturated model excluded at plan | 1+2 |

Shared harness extensions:

- `work_unit_header(id)` — sets `X-Work-Unit-Id`
- `agent_header(name)` — sets `X-Agent-Name`
- `RoutingLoadProfile::gemini_free_slots(n)` — already exists via harness

Unit tests mirror scenarios in `budget_aware/plan/tests.rs` and `budget_aware/memory/tests.rs`.

### D9 — Work-unit route memory (moka)

New `WorkUnitRouteMemory` in `router/budget_aware/memory/` backed by existing
**moka** `Cache` (already in workspace deps for cache/policy):

```text
Key: (agent_name, work_unit_id)   // work_unit_id required; no-op when None
Value: RouteBinding {
  credential_id, model_slug,
}
TTL: 30 min since last insert (moka `time_to_live`)
Max capacity: 10_000 entries (eviction under load)
```

Implementation sketch:

```rust
// memory/registry.rs — thin wrapper, ~40 lines
moka::future::Cache<(CompactString, CompactString), RouteBinding>
```

On plan: if binding exists AND credential not circuit-open AND
`snapshot.headroom(binding) > 0` AND model still ladder-eligible → place binding
as hop 0 with affinity boost (not exclusive lock).

On success: `memory.record(agent, work_unit, binding)` (refreshes TTL).

On failoverable failure on binding hop: `memory.invalidate(agent, work_unit)`.

**Industry analogue:** Portkey [sticky load balancing](https://portkey.ai/docs/product/ai-gateway/load-balancing)
— `hash_fields` + TTL in Redis. Our v1 is the same semantics in-process:
`hash(agent_name, work_unit_id)` affinity with invalidation on failure.

**Alternative rejected:** `Mutex<HashMap>` — contends under 3+ concurrent work units;
moka is already proven in this codebase (`middleware/decision/policy/store.rs`).
**Alternative rejected:** Distributed sticky sessions — v2 Redis; v1 in-process sufficient
for single-pod stage.

### D10 — Quota snapshot at plan time

`QuotaSnapshot::capture(pacing_registry, health, model_states, estimated_tokens)`:

```text
For each (credential, model) pacing scope:
  next_wait = peek_next_wait(tokens)   // sync read, no permit
  daily_headroom = rpd_remaining > 0
  score = 0 if next_wait > max_cooldown_wait OR daily_headroom == false
        else 1.0 / (1 + next_wait.as_secs())
```

Planner excludes candidates with score == 0 before building chain.

Parallel work units: when building hop 0 for three concurrent requests, hash spread
among credentials with score > 0; if only 2 have headroom, third unit's plan starts
with the third-best provider (openrouter) rather than queueing on saturated gemini-9.

**Alternative rejected:** Blocking wait for quota at plan time — preserves existing
`max_cooldown_wait` skip semantics; no new queueing layer.

### D11 — Free-tier catalog as planner input

Planner MUST resolve per-model limits via existing `ProviderLimitCatalog` +
`catalog_limit_resolve::normalize_model_slug` (same path as pacing registry build).

No duplicate limit YAML. Tests assert behavior against embedded catalog values
(e.g. `gemini-3-flash-preview` RPD 20 vs `gemini-2.5-flash-lite` RPD 1500).

### D12 — Invoker concurrency guidance (documented)

Gateway docs SHALL recommend invoker drivers limit concurrent LLM calls to
`min(worker_pool_size, estimated_healthy_free_slots)` when work-unit headers are
present. Not enforced in gateway v1.

### D13 — Per-credential bulkhead (conceptual, v1 light)

Industry pattern ([tower-resilience Bulkhead](https://github.com/joshrotenberg/tower-resilience)):
cap concurrent inflight upstream calls per `(credential_id)` to prevent three work
units from hammering the same RPM-saturated Gemini key in the same tick.

v1 implementation: **no new crate** — planner + `QuotaSnapshot` already avoids
zero-headroom credentials; optional follow-up constant `MAX_INFLIGHT_PER_CREDENTIAL = 2`
as semaphore on dispatch if stage soak still shows triple-hit on live keys.

**Alternative rejected:** Full tower-resilience BulkheadLayer on dispatcher — our
failover loop is custom; planner-first skip is simpler and testable.

### D14 — Rust crate choices

| Need | Crate | Decision |
|------|-------|----------|
| Inbound rate limit | `governor` + `tower_governor` | **keep** — already wired |
| Route memory TTL cache | `moka` | **use** — D9 |
| HTTP retry backoff | `backon` | **keep** — dispatcher layer |
| Per-model pacing | custom `PacingGate` | **keep** — per-(credential, model) RPD/TPM; governor is global GCRA |
| Circuit breaker API | `tower-resilience` | **ideas only** — align thresholds with D2; do not wrap full dispatch stack |
| Per-key limiter | `tokio-rate-limit` | **reject** — duplicates pacing registry |
| API key rotation | `api-key-pool` | **reject** — too naive for ladder/intent |
| Hedge / coalesce | `tower-resilience` | **reject** — doubles LLM quota; work units have distinct prompts |
| Concurrent health map | `dashmap` | **defer** — adopt if mutex profiling shows contention |

### D15 — Rejected pattern: LLM-as-operational-router (hard constraint)

**Definition:** Any architecture where a language model or embedding model participates
in **per-request operational routing decisions** within the gateway hot path.

**Architectural split (invariant):**

```text
Invoker / agent layer (decision plane)     Gateway (control plane)
────────────────────────────────────       ─────────────────────────
Semantic + reasoning allowed               System state only
Intent, tools, agent loop                  Quota, circuit, headroom, scheduling
Latency budget: 100ms–seconds              Latency budget: 1–30ms
Routing = "what does this mean?"           Routing = argmax(score vector)
```

Gateway routing is a **pure function over system state**, not over message semantics.

**Explicitly forbidden in hot path:**

- Credential / API-key / shard selection via LLM or embedding inference
- Quota routing decisions via prompt interpretation
- Circuit-breaker open/close via LLM judgment
- Retry / failover hop ordering via generated text
- Latency-based routing via LLM evaluation
- Per-request LLM call whose purpose is choosing upstream target
- Embedding similarity over **message text** to pick credential or quota slot
- Using semantic prompt content to **infer** live pacing / circuit state

**Allowed:**

- Deterministic `score(c)` from `QuotaSnapshot` + health + memory (D16)
- Rule / heuristic routing (intent from `ModelId`, ladder, cost-class)
- Bandit weight updates on **weights only** (v2 cold/warm path, not per-request LLM)
- Post-hoc LLM analysis in observability / ops tooling (not in request path)

**Rationale:**

| Risk | Effect |
|------|--------|
| Latency | 500–2000ms vs <30ms routing budget |
| Quota amplification | +1 inference per request on already 68% 429 free tier |
| Non-determinism | Same work unit may land on different keys → unstable json |
| Observability | "Model decided" vs auditable `score(c)` vector |
| Feedback loop | Bad route → 429 → worse routing under quota pressure |

**Alternative rejected:** LiteLLM-style optional LLM extension in operational layer —
operational routing stays deterministic in v1 and v2; bandit adjusts weights only.

### D16 — Operational scoring formalism

Operational route selection is **`FEASIBLE(c)` filter → `score(c)` ranking → plan
construction**. Bandit v2 (future) adjusts **weights** \(w_k\), not the function shape.

**Candidate** \(c = (provider, credential\_id, upstream\_model)\).

**State vector** at plan time (read-only snapshot):

```text
s(c) = {
  h_success(c)      ∈ [0, 1]    // rolling success rate (CredentialHealthRegistry)
  h_circuit(c)      ∈ {0, 1}    // circuit-open
  q_headroom(c)     ∈ [0, 1]    // QuotaSnapshot::headroom_score
  q_cooldown(c)     ∈ ℝ⁺        // max(slot, model) cooldown remaining (seconds)
  l_band(c)         ∈ ℕ         // ladder band index (fast < capacity < stability)
  i_tier(c)         tier        // upstream intent tier (catalog capability)
  m_affinity(c)     ∈ {0, 1}    // viable WorkUnitRouteMemory binding
  hash_bias(c)      ∈ ℝ         // caller spread preference from stable_hash
  cost_class(c)     ∈ {free, paid, paid-browser}
}
```

**Hard constraints (feasibility):**

```text
FEASIBLE(c) :=
  ¬h_circuit(c)
  ∧ q_headroom(c) > 0
  ∧ i_tier(c) ≥ floor_tier(contract)     // see D17 — contract, not semantics
  ∧ ladder_eligible(c)
  ∧ payload_capable(c)
  ∧ provider_not_zero_success_dead(c)
```

**Score (v1 deterministic weights):**

```text
score(c) =
    w_health   · h_success(c)
  + w_headroom · q_headroom(c)
  + w_affinity · m_affinity(c)
  + w_hash     · hash_bias(c)
  + w_cost     · cost_class_rank(c)
  - w_cooldown · norm(q_cooldown(c))
  - w_ladder   · l_band(c)              // prefer fast band when feasible
```

Default v1: fixed weights in code (`plan/score.rs`); all terms logged in route trace
for post-mortem.

**Plan construction:**

```text
PLAN =
  sort_desc FEASIBLE candidates by score(c)
  apply spread(work_unit_id) among equal-score credential pools
  append intra-slot ladder UP steps (D6) before cross-provider hops
  truncate to MAX_PLAN_HOPS (7)
```

Failover walks `PLAN` only; on exhaustion, replan once excluding failed hops.

**Bandit v2 extension (non-goal v1):**

```text
w_k ← w_k + η · (reward - baseline)
reward = 1[success] - α·1[429] - β·latency - γ·cost
context = (intent_tier, json_schema, provider_family)   // NOT message text
```

Contextual bandit updates **weights or bias term** only; `FEASIBLE(c)` invariants unchanged.

### D17 — Contract intent, not semantic intent (hard invariant)

The gateway **MUST NOT** infer routing intent from message text, embeddings, or
LLM classification of prompt content.

The gateway **MAY** derive routing constraints from **declared request contract**:

| Input | Source | Operational use |
|-------|--------|-----------------|
| Intent tier / floor | `source_model` → `routing_intent_for_request()` (rule table) | `FEASIBLE(c)` floor filter |
| Strict structured output | `json_schema_required` on payload | floor widening rules per intent spec |
| Payload size | `token_estimate` / payload budget | pacing `peek_next_wait` input |
| Caller identity | `X-Agent-Name`, work-unit headers | spread + route memory |

Invoker chooses `model: gpt-5-mini` → gateway applies **fast-thinking floor** as
config lookup. Gateway does **not** read "classify this ticket" to decide tier.

**Rationale:** Semantic intent belongs to invoker/agent layer (D15 decision plane).
Gateway consumes **contract + system state** only.

### D18 — Operational invariants (hard constraints)

These invariants MUST hold for all v1 and v2 operational routing changes unless an
explicit OpenSpec decision revokes them.

1. **No entropy increase under load** — gateway MUST NOT add exploratory or
   open-ended failover under pressure. Routing decisions come from `FEASIBLE(c)` +
   `score(c)` + bounded `PLAN`, never from trial-and-error over the full candidate pool.

2. **Replayability** — every routing decision MUST be reconstructible offline from
   logged `ReplayRecord` fields (D19) without message semantics or live pacing mutation.

3. **Bounded failover** — failover is plan execution with at most one replan pass;
   upstream attempts per inbound request MUST NOT exceed the stability bound (D19).

4. **No hot-path learning** — per-request model inference or online weight updates
   MUST NOT run in the operational hot path (D15). Bandit v2 updates weights only
   off-request with logged, clamped deltas; `FEASIBLE(c)` predicates are immutable.

**Bandit v2 guardrails (when implemented):**

- MAY adjust `w_*` or per-context bias terms only
- MUST NOT change `FEASIBLE(c)` predicates, plan topology rules, or failover loop structure
- MUST log weight deltas and clamp weights to configured bounds
- MUST NOT run weight updates synchronously during `plan_route_chain`

### D19 — Stability bounds and replay record

**Upstream attempt bound (v1):**

```text
upstream_attempts ≤ MAX_PLAN_HOPS + MAX_REPLAN_HOPS = 7 + 7 = 14
```

Per successful or terminal-failed inbound request. Routing_load scenarios assert
the success path at ≤7; absolute ceiling 14 is a hard invariant (D18).

**Planner latency bound (v1 single-pod target):**

```text
plan_route_chain wall time: p99 < 5ms (snapshot read + score + sort; no HTTP)
```

**Partial-outage behavior:**

When `k` of `N` credentials are circuit-open, plan length is
`min(MAX_PLAN_HOPS, |FEASIBLE|)`. Planner MUST NOT emit hops for infeasible candidates.

**ReplayRecord** (route trace / structured log; v1 log contract, offline replay tooling deferred):

The gateway MUST emit sufficient fields to reconstruct why hop 0 was chosen:

| Field group | Contents |
|-------------|----------|
| Request contract | `source_model`, `json_schema_required`, `agent_name`, `work_unit_id` |
| Snapshot id | `plan_snapshot_ts` (monotonic instant or counter at plan time) |
| Plan meta | `planned_hops`, `plan_rebuilds`, `route_memory_hit`, `route_memory_invalidated` |
| Winner hop 0 | `credential_id`, `model_slug`, `score`, `feasible=true` |
| Score breakdown | `h_success`, `q_headroom`, `q_cooldown_secs`, `m_affinity`, `hash_bias`, `l_band`, `cost_class` |
| Top alternatives | up to 3 next-best feasible candidates with `credential_id`, `model_slug`, `score` (optional v1, recommended) |

Replay uses **contract + score breakdown + health/pacing values at `plan_snapshot_ts`**.
Full historical pacing reconstruction without stored snapshot values is a v2 tooling goal.

**Stage acceptance targets (vs beta.4 baseline):**

| Metric | beta.4 observed | v1 target after change |
|--------|-----------------|----------------------|
| Failover rate (inbound requests with internal failover) | 39% | <15% (soak TBD) |
| Upstream attempts p50 per inbound request | 4.5 | ≤2 |
| HTTP 429 share of upstream outcomes | 68% | <30% (soak TBD) |
| Route memory hit rate (when work_unit_id present) | 0% | >40% after warm-up |

## Architectural invariants (summary)

| Invariant | Enforcement |
|-----------|-------------|
| Operational routing = pure function of system state | D16 `s(c)`, `QuotaSnapshot` |
| No LLM in operational hot path | D15 hard constraint |
| No semantic intent from messages | D17 contract-only |
| No entropy increase / bounded failover | D18 |
| Replayable routing decisions | D19 `ReplayRecord` |
| Stability bounds (≤14 attempts, planner p99) | D19 |
| Stability = ladder UP on same credential before cross-provider | D6 |
| Plan-then-walk, not full-pool walk | D3, max 7 hops |
| Sticky affinity with invalidation | D9 moka memory |
| Bandit adjusts weights only, not FEASIBLE | D18 bandit guardrails |

## Industry alignment (2025–2026 AI Gateway patterns)

This change implements the mainstream **Smart AI Gateway / Plan-Then-Walk Deployment
Pool** style documented by LiteLLM, Portkey, and 2026 gateway architecture surveys.
It is not a novel architecture — it applies industry patterns to **free-tier multi-key
Gemini** with a **client stability-up** policy.

### Pattern map

| Industry name | Reference | Our implementation |
|---------------|-----------|-------------------|
| **Policy engine routing** | LiteLLM router groups, Portkey conditional targets | intent + ladder + cost-class + payload filters |
| **Short fallback chain** | LiteLLM `order` + `fallbacks` | `plan_route_chain` ≤7 hops, single replan |
| **Weighted failover + exclusion** | LiteLLM `enable_weighted_failover`: failed deployment excluded, retry in-group | plan walk + replan excludes failed hops |
| **Sticky session affinity** | Portkey `sticky.hash_fields` + TTL (Redis) | `WorkUnitRouteMemory` on moka (in-process v1) |
| **Health-check driven pool** | LiteLLM health-check routing | `CredentialHealthRegistry` + circuit-open |
| **Usage / headroom routing** | LiteLLM `usage-based-routing`; OpenRouter `:nitro`/`:floor` | `QuotaSnapshot.headroom_score` at plan time |
| **Two-tier quota breaker** | API gateway HLD: RPM 429 ≠ RPD exhaustion | D2 taxonomy + pacing merge (D5) |
| **Stability / quality axis** | Client SLA (not cost-down) | D6 ladder UP before cross-provider |
| **Token-aware quota** | 2026 best practice: estimate → reserve → reconcile | pacing + `token_estimate`; full reserve deferred |

### Architectural style name

Use internally: **Plan-Then-Walk Deployment Pool with Affinity Invalidation**.

```text
LiteLLM model group          Our equivalent
─────────────────────────────────────────────
deployments (keys)      →   BudgetCandidate credentials
routing_strategy        →   plan/score.rs + hash spread
cooldown on 429         →   dynamic cooldown + pacing peek
fallbacks cross-group   →   cross-provider hops in plan
sticky / session        →   WorkUnitRouteMemory (moka)
```

### Gaps vs commercial gateways (documented, not v1)

| Gap | Commercial approach | Our v1 | v2 roadmap |
|-----|---------------------|--------|------------|
| Distributed sticky | Portkey Redis cache | moka in-process | Redis keyed `(agent, work_unit)` |
| Distributed health | LiteLLM Redis cooldowns | in-process registry | shared health via Redis or gossip |
| Latency-based pick | LiteLLM `latency-based-routing` | rank uses historical latency field only | p95 per credential in scorer |
| Hedge requests | tower-resilience Hedge | not used | not planned (2× quota) |
| Semantic cache | gateway cache layer | separate feature | — |

### What we deliberately do not adopt

- **Hedge** — parallel duplicate LLM calls burn free-tier RPD.
- **Coalesce / singleflight** — three invoker work units have different prompts/schemas.
- **Replace `PacingGate` with governor** — wrong granularity (global vs per-model catalog).
- **api-key-pool crate** — no ladder, intent, or stability semantics.
- **LLM-as-operational-router** — hard constraint D15; bandit v2 adjusts weights only (D16).

## Risks / Trade-offs

| Risk | Mitigation |
|------|------------|
| Stale health after pod restart | Cold start: no circuit until min attempts; pacing still protects |
| Stale route memory after quota shift | Invalidate on failure; headroom check at plan time |
| Wrong circuit on transient blip | Require ≥5 samples + <10% success before open |
| Hash collision on few healthy keys | Headroom spread + openrouter fallback in plan |
| Plan too short misses recovery | One replan pass; terminal failure if empty |
| Multi-replica double-spend quota | Document v1 single-pod; shared state is v2 |
| Memory stickiness on degraded key | headroom + circuit check before affinity boost |

## Migration Plan

1. Ship behind no flag (behavior improvement; no API break).
2. Operators: optional work-unit header from invoker drivers; immediate gain from health/circuit.
3. Monitor: `failover_rate`, `upstream_attempts` p50, 429 share, `route_memory_hit` rate.
4. Rollback: revert planner hook — failover_loop uses full `ordered_candidates` again.

## Open Questions

1. Route memory TTL: 30 min default — tune from invoker session length?
2. Circuit-open TTL: 15 min default — tune from stage soak?
3. Add `X-Gateway-Route-Plan` debug header (hop list) for beta ops?
4. Enable `MAX_INFLIGHT_PER_CREDENTIAL` bulkhead after stage if triple-hit persists?
5. v2 Redis sticky: Portkey-compatible `hash_fields` mapping (`X-Agent-Name`, `X-Work-Unit-Id`)?
6. Bandit v2 default weights: soak-tune `w_health` / `w_headroom` vs fixed constants?
