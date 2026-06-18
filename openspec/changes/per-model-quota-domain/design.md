## Context

**Shipped (0.4.2-beta.3):** Gemini per-model scopes, ladder-only walk, catalog verify — implemented
as provider-specific YAML + `ladder_filter` gated on `quota_profile == PerModel`.

**Observed (beta.42 + live curl on `openrouter-default`):**

| Signal | API reality | Gateway today |
|--------|-------------|---------------|
| Nemotron `:free` | 429 `free-models-per-day`, Remaining 0 | Model scope ✓; short RPM cooldown ✗; rank first ✗ |
| gpt-oss `:free` | 200 on same key | Never reached if 402 killed slot or nemotron spam |
| Paid slug | 402 `never purchased credits` | **Project scope → whole slot dead** ✗ |
| Pacing | Per-model RPD at OR | **Per-slot** gate (shared bucket) ✗ |

**Code anchors:** `PacingRegistry::gate_for` branches on `quota_profile`; OpenRouter defaults
`PerSlot`. `quota_scope.rs` line 29: any 402 → `Project`. `sort.rs` alphabetical model tie-break.
`budget_probe` skips paid pre-dispatch but reactive 402 still poisons walk.

## Goals / Non-Goals

**Goals:**

1. **Single domain model** — `ProviderQuotaProfile::PerModel` drives pacing scope, ladder filter,
   exhaustion scope overrides, and rank — for **any** provider that opts in via YAML.
2. **OpenRouter consumer** — first provider after Gemini using the generalized path; per-slug
   `rpd: 50` in catalog; free ladder ordering; 402/429 fixes.
3. **Stability = escalate up** — ladder `stability` band on same credential before inter-slot /
   cross-provider; never downgrade below `floor_tier`; client-ordered stability over cost when
   intra-slot models remain.
4. **Architectural tests** — acceptance matrix keyed by failure signal × profile, not provider name.

**Non-Goals:**

- Rewriting `per-slot` / `per-session` providers (Mistral, ChatGPT Web).
- Runtime dynamic OpenRouter catalog refresh every request.
- Paid OpenRouter credits / billing purchase automation.
- Distributed pacing across gateway replicas.

## Decisions

### D1 — Domain entry: `quota-profile: per-model` is the only switch

No `if provider == OpenRouter` branches. Opt-in via `provider-limits.yaml`:

```yaml
openrouter:
  quota-profile: per-model
```

Activates: `PacingScope::CredentialModel`, `ladder_filter`, ladder rank, per-model cooldown map.
**Rejected:** OpenRouter-only module — duplicates Gemini path.

### D2 — Pacing key = `(credential_id, wire_slug)`

```text
PerModel → gate key: "openrouter-default::openai/gpt-oss-120b:free"
PerSlot  → gate key: "openrouter-default"   (legacy)
```

Limits resolve per slug via `catalog_limit_resolve` (explicit model entry or suffix rule with
**per-model gate**, not shared daily counter).

### D3 — Failure signal taxonomy (classify → scope → cooldown)

```text
┌──────────────────┬─────────────┬──────────────────┬─────────────────────┐
│ Signal           │ FailoverClass│ Scope (per-model)│ Cooldown            │
├──────────────────┼─────────────┼──────────────────┼─────────────────────┤
│ 429 RPM          │ Transient   │ Model            │ Retry-After / 60s   │
│ 429 model RPD    │ QuotaExhaust│ Model            │ X-RateLimit-Reset * │
│ 429 free-models- │ QuotaExhaust│ Model            │ header reset UTC    │
│     per-day      │             │                  │                     │
│ 402 never bought │ Transient   │ Model (NEW)      │ long model / 1h     │
│ 402 billing cap  │ QuotaExhaust│ Project          │ slot + skip siblings│
│ 404 / unsupported│ Transient   │ Model            │ quota-exhausted 1h  │
│ 503 high demand  │ Overload    │ Model walk **    │ short slot cd also  │
└──────────────────┴─────────────┴──────────────────┴─────────────────────┘
```

\* Parse `X-RateLimit-Reset` ms epoch when body contains `free-models-per-day`.
\** Walk continues on same credential (same as Gemini D4).

**Rejected:** 402 → Project for all PAYMENT_REQUIRED — caused 0% OR success on beta.42.

### D4 — Ladder-only walk (generic)

`provider-ladders.yaml` bands: `fast` → `capacity` → `stability`. `ladder_filter` applies when
`quota-profile: per-model` **and** ladder exists for `(provider, tier)`.

OpenRouter free ladder (initial):

```yaml
openrouter:
  free:
    fast:      [openrouter/free, openai/gpt-oss-120b:free]
    capacity:  [qwen/qwen3-next-80b-a3b-instruct:free]
    stability: [openai/gpt-oss-120b:free]  # larger than nemotron nano
    deprioritized: [nvidia/nemotron-3-nano-30b-a3b:free]  # last resort
```

Nemotron in `deprioritized` or end of stability — not alphabetical first.

### D5 — Rank order replaces alphabetical tie-break

Within same `budget_rank` band, sort by:

1. `ladder_rank` (band index, deprioritized last)
2. `intent_proximity` (unchanged)
3. `json_schema_rank` (unchanged)
4. **Remove** raw `model.to_string()` alphabetical sort

### D6 — Budget probe + 402 defense in depth

1. Pre-dispatch: `should_skip_candidate` when `blocks_paid_route(model)` (exists).
2. Reactive 402 on unpaid slug: `Model` scope + `record_payment_required` only for **paid**
   slug attempts, not whole slot.
3. Never map client alias `openai/gpt-5.4-nano` to OpenRouter wire without `:free` on free-tier
   accounts (mapper already has `model_id` on budget-aware dispatcher — add regression test).

### D7 — Test architecture (use-case driven)

| Layer | Proves |
|-------|--------|
| Unit `quota_scope` | 402 unpaid → Model; free-models-per-day → QuotaExhausted |
| Unit `pacing/scope` | OR nemotron gate ≠ gpt-oss gate |
| `routing_load` | `or_nemotron_429_then_gpt_oss_200_same_slot` |
| `routing_load` | `or_402_paid_does_not_kill_free_siblings` |
| `intent_acceptance` | fast-thinking → gpt-oss before groq when nemotron exhausted |
| Emulator | `402-never-purchased`, `429-free-models-per-day` wire bodies |
| Stage smoke | provider-stats: OR 200% > 0 when Gemini healthy |

## Risks / Trade-offs

| Risk | Mitigation |
|------|------------|
| OpenRouter changes per-model limits | Fixture + verify task; `last_verified_at` |
| Ladder too narrow (no nemotron) | `deprioritized` band still tries nemotron last |
| Stability uses larger model = more latency | Client explicit stability preference; log escalation phase |
| Gemini regression | Re-run existing `routing_load` gemini scenarios in CI |

## Migration Plan

1. Land domain generalization (no YAML change) — Gemini unchanged behavior.
2. Add OpenRouter `quota-profile` + ladder + per-slug limits.
3. Deploy beta.4 to stage; smoke provider-stats OR success rate.
4. Rollback: remove `quota-profile` from openrouter → legacy per-slot.

## Open Questions

- Whether `openrouter/free` router slug counts as separate RPD bucket (verify with fixture).
- GitHub Models free slugs as second consumer in same release or follow-up.
