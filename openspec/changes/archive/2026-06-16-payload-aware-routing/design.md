## Context

`autodefault` is a `budget-aware-capability-after` router with `free-up`
cascade. For Sales-QA it receives fat json_schema requests (one report observed
~60k input; logs show up to ~128k input). Today the router builds an ordered
candidate list (`ordered_candidates`) and walks it in `run_failover_candidates`,
dispatching to each candidate and failing over on any failoverable status. It
has **no notion of payload size** when choosing or skipping candidates.

### Evidence (`llm-gateway:0.3.0-beta.12`, ~2h window)

Provider × status (observed):

| provider | status | count | meaning |
|---|---|---|---|
| gemini | 429 | 42 | RPM + daily `RESOURCE_EXHAUSTED` mixed |
| gemini | 503 | 31 | free-tier overload |
| gemini | 200 | 9 | success (~11% of 82 attempts) |
| groq | 413 | 33 | `TPM: Limit 30000, Requested 60946` |
| openrouter | 400 | 23 | `max context 131072 … requested ~128k input + 4000 output` |
| openrouter | 200 | 16 | success |
| cloudflare | 429 | 24 | `used up your daily free allocation of 10,000 neurons` |
| mistral | 429 | 15 | `Service tier capacity exceeded` |
| cerebras | 429 | 14 | free TPM 60k vs large input |
| mistral | 200 | 11 | success |

Two report claims **disproved**:
1. OpenRouter `400` is **context-length overflow**, not a json_schema failure.
2. Input is **not** uniformly ~60k; the same batch shows groq measuring 60946
   while OpenRouter measures 128212 input tokens. Token estimation must be real.

### Current code constraints

- `extract_requirements` hardcodes `min_context_tokens: None`
  (`router/capability/mod.rs`), so the existing `supports()` context-window
  filter is dead.
- Provider `context_window` values in `capability/providers.rs` are coarse and
  partly wrong for filtering (groq `8000`, openrouter openai/* `128000` vs real
  `131072`); per-model TPM lives in `provider-limits.yaml` but is used only for
  RPM pacing (`pacing/limits.rs`), never for token-based skipping.
- Gemini sibling failover is mandated by the shipped `gemini-free-multi-account`
  spec; cooldown is applied only *after* a failure, so within one request all 4
  free keys are tried, and concurrent batch requests race before cooldown gates.

## Goals / Non-Goals

**Goals:**
- Stop dispatching requests to candidates that cannot physically accept them
  (context window / per-minute token cap), pre-flight.
- Stop burning all free Gemini slots on daily-quota / overload errors within a
  single request; preserve sibling failover for transient RPM limits.
- Guarantee one paid Gemini attempt after free is exhausted/skipped.
- Order json_schema requests toward reliably-capable providers.
- Give ops per-credential + quota-metric visibility and a per-request hop/seconds
  summary.

**Non-Goals:**
- Prompt compression / truncation of oversized payloads (caller's concern; we
  only route, we do not mutate request bodies).
- Changing budget/cost ranking semantics or the `free-up` cascade.
- Live runtime quota polling (OpenRouter `/key`, OpenAI headers) — out of scope;
  filtering uses static catalog + observed cooldowns.
- Touching `health.rs` locking (owned by `health-monitor-concurrency`).

## Decisions

### D1 — Real provider-aware tokenizer (chosen) over byte/char heuristic

Estimate input tokens with a real tokenizer rather than a `bytes/4` heuristic.
Rationale: the 60k↔128k discrepancy shows heuristics misclassify borderline fat
payloads exactly where the decision matters (near 131072). A BPE tokenizer
(tiktoken `o200k_base`/`cl100k_base` family) gives a stable estimate that
generalizes across OpenAI-compatible providers; provider-specific divergence is
absorbed by a conservative safety margin (D3).

- Alternatives: (a) `bytes/4` heuristic — cheap, no deps, but unreliable at the
  boundary; rejected as primary. (b) call each upstream's `/tokenize` — accurate
  but adds a network hop per candidate, defeating the purpose; rejected.
- Estimate counts the **full serialized prompt including the json_schema** (the
  fat schema is a large share of tokens) plus tool definitions.

### D2 — Effective window = `min(context_window, per-model TPM cap)` minus output reservation

A candidate is eligible only if
`estimated_input + reserved_output <= effective_window(candidate)` where
`effective_window = min(context_window, tpm_cap_for_tier_model)` and
`reserved_output = max_tokens` from the request (fallback to a configured
default, matching the 4000 OpenRouter reserves). This single rule covers both
the `413` (TPM) and `400` (context) failure modes with one check.

- `min_context_tokens` is repurposed to carry `estimated_input + reserved_output`
  so the existing `supports()` gate becomes live; per-model TPM is surfaced from
  `provider-limits.yaml` into the candidate's effective window.
- If **no** candidate fits, do not silently drop to `ProviderNotFound`: keep the
  largest-window candidate(s) as a best-effort tail so oversized requests still
  get one honest attempt + a clear upstream error, rather than an opaque 500.

### D3 — Conservative safety margin, fail-open on unknown limits

Apply a margin (e.g. treat effective window as `window * (1 - margin)`) to
absorb tokenizer error and provider accounting differences. When a provider's
context/TPM is `Unknown` in the catalog, **do not filter it out** (fail-open) —
only filter when we have a concrete limit that the estimate provably exceeds.
This keeps the change safe next to in-flight provider additions.

### D4 — Quota-aware Gemini sibling skip via retry classification

Extend `retry_after::classify` to expose the 429 sub-kind (transient RPM vs
`RESOURCE_EXHAUSTED` daily/quota) and treat `503` as overload. The failover loop
consults this: for transient RPM it continues to the next free sibling (current
behavior, satisfies `gemini-free-multi-account`); for daily-quota/overload it
marks **all remaining same-provider free siblings** as skipped for this request
and jumps to the paid slot / next provider.

- This refines, not contradicts, `gemini-free-multi-account`: sibling failover
  is preserved for the transient case the spec was written for.

### D5 — Structured-output ordering as a secondary rank signal

For `json_schema_required`, add a ranking signal that prefers providers with
proven strict-schema reliability (config-driven allowlist already partly encoded
in `capability/providers.rs`) and demotes providers observed to reject the
schema, without breaking budget-first ordering. Strict-schema *validation* of
responses already exists (`structured_output.rs`); this only changes *ordering*.

### D6 — Observability: attributes + trace summary

Add `credential` and `quota_metric` (`rpm|tpm|rpd|context|overload`) attributes
to failover/cooldown counters, and emit one structured per-request summary event
(`hops`, `duration_ms`, `terminal_provider`, `terminal_status`, `skipped_*`) at
the end of `run_failover_candidates`.

## Risks / Trade-offs

- [Tokenizer estimate diverges from provider accounting near the boundary] →
  conservative margin (D3) + fail-open on unknown limits; calibrate margin from
  observed `413/400` bodies post-deploy.
- [Over-filtering removes a candidate that would have succeeded] → best-effort
  tail (D2) guarantees at least one attempt; margin tuned conservatively.
- [Tokenizer adds per-request CPU cost] → estimate once per request (body is
  already fully buffered in `dispatch.rs`), not per candidate; cache by body
  hash if needed.
- [New dependency footprint] → pick a pure-Rust BPE crate; gate behind the
  existing routing module, no new runtime services.
- [Concurrent batch still races before cooldowns set] → D4 reduces in-request
  waste; cross-request racing is mitigated but not eliminated (acceptable;
  separate from this change).
- [Coordination with in-flight changes] → confined to routing/metrics files;
  no edits to provider catalog ports or `health.rs`.

## Migration Plan

1. Land token estimation + effective-window filter behind conservative margin
   (fail-open); monitor `413/400` hop counts in metrics/logs after rollout.
2. Land quota-aware Gemini skip + paid fallback; verify Gemini attempts per
   request fall from ~4 to ≤2 when daily quota is exhausted.
3. Land structured-output ordering + observability.
4. Rollback: filtering is additive — disabling the margin/feature reverts to
   current walk-all behavior with no schema/data migration.

## Open Questions

- Exact tokenizer crate and default `reserved_output` when `max_tokens` absent
  (proposed default 4000 to mirror OpenRouter) — confirm during apply.
- Whether per-model TPM caps should be promoted into `capability/providers.rs`
  context windows or read live from the `provider-limits` catalog at rank time
  (leaning catalog-at-rank to keep one source of truth).
