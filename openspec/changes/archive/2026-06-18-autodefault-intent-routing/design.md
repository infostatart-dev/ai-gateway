## Context

Autodefault builds a **global candidate pool** in
`budget_aware/factory.rs` (every resolved credential × provider catalog model),
ranks by cost-class and provider priority, then **filters** through
`matches_source_model` in `selection.rs` / `selection_mode.rs` — a gate that
requires the candidate's upstream model to appear in `model-mapping.yaml` for
the client's alias.

Clients use OpenAI-style model names as **intent**, not SKU:

| Client says | Means |
|-------------|-------|
| `gpt-5-nano`, `gpt-5-mini` | **fast-thinking** — быстрые ответы, но «думающие»; mini ≡ nano |
| `gpt-5` (plain), `o1`, `o3` | **deep** — хорошо думает, качество важнее скорости |
| payload: `json_schema` | hard filter — только upstream с json_schema |
| payload: plain | wider pool — json и non-json upstream в intent band |
| (implicit) | **stability** — можно эскалировать вверх, нельзя вниз |

**Observed bug (code today):** `reasoning_preferred_for_model_name` in
`router/capability/mod.rs` treats `gpt-5-nano`, `gpt-5-mini`, and substring
`gpt-5` as reasoning-preferred. Because `gpt-5-nano`.contains(`gpt-5`) is
true, nano requests boost reasoning models in `capability_fit_score`, rank
deep/large models ahead of fast scouts, and after cooldown failover the walk
ends on the **largest** capable upstream — opposite of client intent.

Web providers (`chatgpt-web`, `deepseek-web`) already bypass binding in
`matches_source_model` (`return true`). API providers do not.

Related in-flight change `autodefault-credential-pools` patches mapping parity
(symptom). This change addresses the root cause for autodefault.

## Goals / Non-Goals

**Goals:**

- Interpret client `model` + payload as `RoutingIntent` (latency/reasoning tier,
  hard requirements, stability floor).
- Autodefault uses **intent pool selection** instead of per-alias binding.
- **Asymmetric stability:** escalate to larger/deeper upstream when fast/cheap
  paths fail; never de-escalate below client intent floor.
- Fix reasoning/latency misclassification for `gpt-5-*` family.
- Preserve **strict binding** for operator-configured routers.
- Compose with existing cost-class ranking, payload-aware filter, tier cascade,
  credential round-robin, and failover loop.

**Non-Goals:**

- Changing OpenAI API surface or client SDK contracts.
- LLM-based intent classification (name heuristics + payload only in v1).
- Removing `model-mapping.yaml` (still used for strict mode and ops docs).
- Auto-syncing mapping lists across aliases (intent pool makes this unnecessary
  for autodefault).

## Decisions

### D1 — `RoutingIntent` struct (new module `router/intent/`)

```rust
pub enum IntentTier { Fast, FastThinking, Standard, Deep }

pub struct RoutingIntent {
    pub preferred_tier: IntentTier,
    pub floor_tier: IntentTier,
    pub escalation_ceiling: IntentTier,
    // hard reqs in RequestRequirements — json_schema toggles pool width, not tier
}
```

Extract from source model name (longest match first to fix substring bug):

| Pattern (case-insensitive) | preferred | floor | escalation ceiling |
|----------------------------|-----------|-------|----------------------|
| `gpt-5-nano`, `gpt-5-mini`, `gpt-5.4-nano`, `gpt-5.4-mini` | FastThinking | FastThinking | Deep |
| `flash`, `lite`, `instant`, `8b-instant` (non gpt-5 family) | Fast | Fast | Deep |
| `small`, `haiku` (non gpt-5 family) | FastThinking | FastThinking | Deep |
| `gpt-5` (no nano/mini suffix), `o1`, `o3`, `o4`, `reasoner`, `thinking`, `opus`, `pro` | Deep | Deep | Deep |
| default / unknown alias | Standard | Standard | Deep |

**Mini ≡ nano:** same intent tier; alias string does not narrow the pool in
intent mode.

Payload shape (does **not** change intent tier):

| Payload | Pool filter after intent band |
|---------|-------------------------------|
| `json_schema` strict | only upstream with `supports_json_schema` |
| plain (no json_schema) | json and non-json upstream in fast-thinking band |

Ranking among survivors: cost-class → budget-rank → cooldown availability →
provider priority (existing autodefault stack).

### Canonical acceptance matrix (must pass in CI)

| Case | Model | Payload | Intent | Eligible pool | Rank by |
|------|-------|---------|--------|---------------|---------|
| A | gpt-5-mini | json strict | fast-thinking | json-capable, fast-thinking band | free → available |
| B | gpt-5-mini | plain | fast-thinking | json + non-json, fast-thinking band | free → available |
| C | gpt-5-nano | json strict | fast-thinking | same as A | free → available |
| D | gpt-5-nano | plain | fast-thinking | same as B | free → available |

**Rejected:** single `reasoning_preferred: bool` — cannot express fast vs deep
vs stability asymmetry.

### D2 — Upstream `intent_tier` on `ModelCapability`

Add `intent_tier: IntentTier` to `ModelCapability` (`capability/mod.rs`),
populated from:

1. Explicit `intent-tier` in `providers.yaml` per model (preferred), or
2. Derived helper `default_intent_tier(provider, model_slug)`:
   scout / gpt-oss / nemotron / economy reasoning slugs → **FastThinking**
   dumb flash / instant / 8b → **Fast**
   sonnet / gpt-5 / o1 / 70b+ → **Deep**

Used for floor filter and rank tiebreak — **not** for binding.

**Rejected:** infer tier only at rank time from model name string — duplicates
logic and misses provider-specific slugs (`llama-4-scout` vs `gpt-oss-120b`).

### D3 — Router selection mode: `source_model_selection`

Add to `RouterConfig` (`config/router.rs`):

```yaml
source-model-selection: strict | intent  # default: strict
```

- `build_autodefault_router_config` in `read.rs` sets **`intent`**.
- All other routers keep **`strict`** (current behavior).

In `BudgetAwareRouter`, store mode and branch in selection:

| Mode | Candidate filter |
|------|------------------|
| `strict` | `matches_source_model` (today) |
| `intent` | `intent_tier >= floor` AND hard capability/payload filters; **skip** mapping gate |

Web session providers remain always eligible when capability matches (same as
today's permissive binding).

### D4 — Rank order in intent mode

Preserve existing sort keys from `rank_score.rs` / `capability_fit_score`:

1. cost-class (`free` → `paid` → `paid-browser`)
2. effective budget-rank (incl. cooldown penalty)
3. **intent proximity** — candidates closer to `preferred_tier` rank first
   (Fast before Standard before Deep when client asked nano)
4. json_schema_rank / capability fit (revised — see D5)
5. provider round-robin

**Rejected:** replacing cost-class with intent-first — free-up cascade policy
must remain primary economic guard.

### D5 — Fix `reasoning_preferred` / replace with intent proximity

- Remove `gpt-5-nano`, `gpt-5-mini`, bare `gpt-5` substring rule from
  `reasoning_preferred_for_model_name`.
- Deep keywords only match when **not** preceded by nano/mini/fast suffixes
  (longest-match-first table in D1).
- `capability_fit_score`: replace reasoning bool boost with
  `intent_proximity_score(preferred_tier, candidate.intent_tier)` — fast
  candidates score higher for nano, deep candidates for plain gpt-5.

This directly fixes: nano client no longer walks all reasoning models first.

### D6 — Asymmetric stability escalation (failover widening)

Two-phase candidate ordering within intent mode:

```
Phase A (preferred band):
  candidates where intent_tier == preferred_tier
  → rank → payload filter → failover loop

Phase B (stability escalation — only if Phase A exhausted):
  candidates where floor_tier < intent_tier <= escalation_ceiling
  ordered: Standard before Deep when preferred was Fast
  → same rank/failover

Never Phase C below floor:
  gpt-5 (Deep floor) MUST NOT include Fast/Standard scouts even if free
```

Implementation hook: extend `ordered_candidates` in `selection_mode.rs` to
return `Vec<BudgetCandidate>` already segmented, or tag candidates with
`selection_phase` and let `failover_loop.rs` advance phase when the current
segment is exhausted (all credentials failed or skipped).

**Rationale:** matches user story — nano may end on larger model for stability,
but gpt-5 must never silently become scout.

**Rejected:** single flat pool sorted by tier — would interleave deep models
into nano's first hop (regression).

### D7 — Payload best-effort tail respects intent floor

`payload.rs` best-effort when no candidate fits: pick largest window among
candidates **at or above floor tier**, never below. If none exist, return
`ProviderNotFound` rather than downgrade.

### D8 — Observability

Extend route trace / response headers (`middleware/response_headers.rs`):

- `X-Routing-Intent-Tier: fast|standard|deep`
- `X-Routing-Selection-Phase: preferred|escalated`
- existing routed model identity headers unchanged

### D9 — Relationship to `model-mapping.yaml`

Strict routers: unchanged.

Autodefault intent mode: mapping is **not** a selection gate. Optional future:
mapping seeds `intent_tier` hints — out of scope v1.

Deprioritize `autodefault-credential-pools` mapping parity tasks (D2–D6 there)
once this lands; keep Gemini×16 and DeepSeek×2 pool expansion.

## Risks / Trade-offs

| Risk | Mitigation |
|------|------------|
| Nano client gets slow deep model after escalation | Phase A tries all fast first; headers show `escalated`; ops can tune ceiling |
| Unknown alias defaults to Standard | Document; operator can set strict router for exotic aliases |
| intent_tier mis-tag on new upstream | Explicit yaml override; unit tests per catalog model |
| Phase B doubles failover latency | Only entered after Phase A exhausted — acceptable for stability |
| Human router regression | strict remains default except autodefault |

## Migration Plan

1. Land intent module + ModelCapability field + config knob (no behavior change
   until autodefault wired).
2. Switch autodefault to `intent` mode behind tests.
3. Fix `reasoning_preferred_for_model_name` (safe standalone fix).
4. Update `docs/routing.md`; note mapping parity ops burden reduced.
5. Rollback: set autodefault to `strict` in `read.rs` one-liner.

## Open Questions

- Escalation ceiling for Deep floor clients when no Deep candidate exists:
  fail hard vs best-effort Standard? Proposal: **fail** — no downgrade.
- Resolved: `gpt-5-mini` and `gpt-5-nano` share **fast-thinking** intent (same
  tier, same pool); payload shape (plain vs json strict) is the only difference
  in capability filter width.
