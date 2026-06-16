## Context

Sidecar mode builds router `autodefault` with strategy
`budget-aware-capability-after`, decision `free-up`, and a fixed provider list in
`autodefault_provider_order()`. Candidate ranking combines:

1. `default_provider_budget_rank()` — hardcoded per provider in `rank.rs`
2. `credential.budget_rank` — from `credentials.yaml`
3. `model-mapping.yaml` — first matching target per source model when binding

### Current problems (as of beta.15)

| Issue | Today | Why it hurts |
| --- | --- | --- |
| Browser-first | `chatgpt-web` and `deepseek-web` are ranks **0** and listed **first** | Paid/risky browser sessions win before free API keys |
| Nano mapping | `gpt-5.4-nano` → Claude/Gemini Pro first | Budget router burns paid paths for the default nano model |
| Mini mapping | `gpt-5-mini` → OpenRouter `:free` first | Correct pattern, but CLI examples use mini not nano |
| Tier metadata | `credentials.yaml` has `tier: free \| tier-3 \| pro` | Not used consistently for cross-provider sort |
| Web bypass | `chatgpt-web` / `deepseek-web` skip model-map checks | Any source model routes to web providers when they rank first |

Operator context: ChatGPT Web is a **~$20/mo subscription** with abuse-sensitive
pacing; it should be a **last resort**, not the default path.

## Research inputs (2026)

### Industry gateway pattern

Production routers separate **cascade** (cost: try cheap first) from **fallback**
(reliability: try next on 429/5xx). LiteLLM documents an explicit `order` field
(lower = higher priority) within a model group before cross-model fallbacks.
Braintrust/OpenRouter guidance for 2026: free-tier keys first, paid escalation on
quota or hard failures — not browser sessions as primary path.

### Tier model in reference implementations

A mature auto-router uses three economic tiers — `free`, `cheap`, `premium` —
resolved from catalog cost data and explicit overrides. Tool-carrying requests
get a minimum floor of `cheap` because function-calling reliability beats raw
token price. Scoring weights cost ~15%, health ~20%, quota ~15%.

We map this to four **cost-classes** in ai-gateway (browser split out):

| Our cost-class | Analog |
| --- | --- |
| `free` | API keys at $0 marginal |
| `paid` | Metered / tier-3 API |
| `paid-browser` | Subscription browser (ChatGPT Plus/Pro) |

`subsidized` is **not used in v1** — `github-models` and `opencode` stay `free`
to avoid an extra band operators must reason about.

### Documented free-tier headroom (June 2026 catalog snapshot)

Approximate recurring monthly token budgets used to sanity-check provider
priority within the `free` band:

| Provider | Documented monthly tokens |
| --- | ---: |
| mistral | ~1.0B |
| cloudflare | ~122M |
| gemini (AI Studio free) | ~60M |
| cerebras | ~30M |
| github-models | ~18M |
| groq | ~15M |
| openrouter | ~1.2M |

OpenRouter has the smallest published pool but the richest `:free` model variety;
it stays high in the list for mapping breadth, not raw quota.

### Web providers

| Provider | Economics | Automation notes |
| --- | --- | --- |
| `deepseek-web` | $0 account | Session + PoW; upstream rejects `tools[]`; good chat quality |
| `chatgpt-web` | ~$20/mo subscription | Abuse-block cooldown hours; pacing like one browser tab |

**Resolved:** Gemini free API slots precede `deepseek-web` — API keys scale
better (multi-slot, no PoW, tooling path). `deepseek-web` precedes paid
`gemini-default` as a free quality bridge. `chatgpt-web` is always last.

### GitHub Models / OpenCode

GitHub Models: free rate-limited prototyping (`models:read`), catalog marks ToS
**caution** for proxy/resale — operational note only, cost-class **`free`**.

OpenCode: free tier; some third-party catalogs mark ToS **avoid** for proxy use —
keep in autodefault but document operator responsibility.

## Goals / Non-Goals

**Goals:**

- Cost-class-first ranking: `free` API → `paid` API → `paid-browser`.
- Move `chatgpt-web` to the **end** of autodefault.
- Place `deepseek-web` **after** free API + Gemini free, **before** paid API.
- Align `gpt-5.4-nano` model bindings with cost-first YAML order.
- Canonical autodefault client model: **`openai/gpt-5.4-nano`** (with provider prefix).
- Policy in embedded YAML; secrets in env only.

**Non-Goals:**

- Replacing `payload-aware-routing` (beta.16).
- Live pricing/quota polling.
- Removing `chatgpt-web` from autodefault (stays as last-resort fallback).
- `subsidized` cost-class band in v1.
- Opt-out env for ChatGPT Web in v1 (session file gating is sufficient).

## Decisions

### D1 — `cost-class` on credential slots (catalog, not env)

| cost-class | Meaning | Examples |
| --- | --- | --- |
| `free` | $0 marginal API or free browser | all `tier: free` slots, `deepseek-web-default` |
| `paid` | Metered / tier-3 API | `gemini-default`, `openai-default`, `anthropic-default` |
| `paid-browser` | Paid subscription browser | `chatgpt-web-default` |

Derivation when omitted:

- `tier: free` → `free`
- `tier: tier-3` / `developer` → `paid`
- `chatgpt-web` session → `paid-browser`
- `deepseek-web` session → `free` (free account; ordered late within `free` band)

Secrets: `AI_GATEWAY_CREDENTIAL_*` only. Optional:
`AI_GATEWAY_AUTODEFAULT_DEFAULT_MODEL` for banner/docs example model.

### D2 — Autodefault provider order

Within `free` cost-class (availability-gated):

1. `opencode`
2. `openrouter`
3. `github-models`
4. `mistral`
5. `groq`
6. `cerebras`
7. `cloudflare`
8. `gemini` (free slots first via `budget-rank`; `gemini-default` is `paid`)
9. `deepseek-web`

Then `paid` cost-class:

10. `anthropic`
11. `openai`

Then `paid-browser`:

12. `chatgpt-web` — **last**

### D3 — Ranking formula

```
effective_sort_key =
  (cost_class_rank, credential.budget_rank, provider_priority, cooldown_penalty)
```

| cost-class | rank base |
| --- | ---: |
| `free` | 0 |
| `paid` | 200 |
| `paid-browser` | 300 |

(`subsidized` at 100 reserved for a future change if needed.)

### D4 — Model binding for default nano

Reorder `gpt-5.4-nano` to mirror `gpt-5-mini` cost-first pattern:

```yaml
gpt-5.4-nano:
  - openrouter/openai/gpt-oss-120b:free
  - opencode/nemotron-3-ultra-free
  - opencode/mimo-v2.5-free
  - opencode/deepseek-v4-flash-free
  - openrouter/qwen/qwen3-next-80b-a3b-instruct:free
  - github-models/openai/gpt-4o-mini
  - groq/llama-3.1-8b-instant
  - cerebras/gpt-oss-120b
  - mistral/mistral-small-latest
  - cloudflare/@cf/meta/llama-3.2-3b-instruct
  - gemini/gemini-2.0-flash
  # paid fallbacks after free band exhausted:
  - anthropic/claude-3-5-haiku
  - deepseek/deepseek-chat
```

Mapping walk skips providers without resolved credentials (existing behavior,
explicitly tested).

Web providers keep permissive `matches_source_model` but rank last via D2/D3.

### D5 — Canonical default model

**`openai/gpt-5.4-nano`** everywhere in autodefault examples (CLI banner, routing
docs). Override: `AI_GATEWAY_AUTODEFAULT_DEFAULT_MODEL`.

### D6 — Coordination with beta.16 (landed)

**Status (2026-06-16):** `payload-aware-routing` is implemented in code
(`de72f37`, workspace **`0.3.0-beta.16`**). Beta.17 changes **priority among
survivors** after payload/capability filtering — it does not replace beta.16.

Selection pipeline today (`selection_mode.rs`):

1. `rank_candidates` — budget rank + `json_schema_rank` + capability fit
2. `filter_payload_capable` — TPM/context window + tools/json_schema gates
3. `credential_round_robin` — balance within final list

Cost-class MUST be injected at step 1 (`effective_budget_rank`) so step 2
preserves relative order of survivors.

## Implementation status (2026-06-16)

| Item | Status |
| --- | --- |
| `payload-aware-routing` (beta.16) | **Landed** — token estimate, payload filter, Gemini quota sibling skip, `json_schema_rank`, route trace |
| `autodefault-routing-priority` (beta.17) | **Not landed** — `read.rs` still browser-first; `rank.rs` still `chatgpt-web`/`deepseek-web` at 0; `gpt-5.4-nano` mapping still paid-first |
| Workspace version | **`0.3.0-beta.16`** in `Cargo.toml` (beta.17 bump deferred; CI fix in parallel) |
| `CHANGELOG.md` | **Stale** — last entry `0.3.0-beta.11`; beta.12–16 shipped without notes; beta.17 TBD |

**Release-notes debt:** when closing beta.17, backfill `CHANGELOG.md` for
**beta.12 through beta.17** in one pass (see tasks §7). Do not ship beta.17
with only a single-line entry while beta.12–16 remain undocumented.

## Risks / Trade-offs

### Operator / policy

- **ChatGPT-last breaks browser-first operators** — anyone with
  `CHATGPT_BROWSER_CLI` set today gets ChatGPT Web as autodefault primary;
  beta.17 inverts that. Mitigation: changelog, banner, note in `docs/routing.md`.
- **DeepSeek before paid Gemini but after Gemini free** may miss “best free
  quality” for some prompts → revisit with stage metrics.
- **YAML policy less runtime-flexible than env** → GitOps-friendly; aligns with
  operator preference.

### Interaction with beta.16 (payload-aware)

- **Best-effort tail ignores cost-class.** When every candidate fails the payload
  filter, `filter_payload_capable` relaxes `min_context_tokens` and
  `keep_largest_effective_window` retains only providers with the **largest
  effective window** — no cost-class tiebreak. Fat prompts can still jump to
  paid large-context (Anthropic/OpenAI) even when free providers were skipped for
  TPM, not quota. Mitigation: document; optional follow-up — cost-class
  tiebreak among equal-window survivors.
- **`json_schema_rank` must stay subordinate to cost-class.** Today
  `sort.rs` applies `json_schema_rank` after `effective_budget_rank`. If
  cost-class is only wired into `autodefault_provider_order()` but not
  `effective_budget_rank`, structured-output requests can still reorder free
  providers incorrectly. Mitigation: task 2.3 + test with `json_schema_required`.
- **Tools requests never reach `deepseek-web`.** Catalog marks
  `supports_tools: false`; capability filter drops it. Tool-carrying autodefault
  traffic skips DeepSeek Web entirely and may escalate to paid API or (last)
  ChatGPT Web — expected; document in `docs/routing.md`.
- **Gemini quota sibling skip unchanged.** Beta.16 skips same-provider free
  siblings on `QuotaExhausted`/`Overload` by matching `credential_budget_rank`.
  Paid `gemini-default` (higher rank, `paid` cost-class) remains as fallback —
  compatible with beta.17 ordering. Risk: if cost-class sort is wrong, paid
  Gemini may be tried before `deepseek-web` free bridge — covered by task 5.3.

### Credential / secrets surface

- **`chatgpt-web-default` slot missing** from `credentials.yaml` today;
  autodefault gates on `CHATGPT_BROWSER_CLI` only (`session_file_available()`).
  Beta.17 task 1.2 adds the slot; until then cost-class `paid-browser` is
  theoretical. Cross-change: `credential-secrets-local` (beta.18) expects
  `chatgpt-web-default.session-file` in secrets YAML.
- **`github-models` budget rank drift.** `default_provider_budget_rank` maps
  unknown `Named` providers to **25**, so github-models currently sorts like a
  mid-tier provider despite free tier — must be fixed in task 2.1.

### ChatGPT Web stabilization (beta.13)

- Moving ChatGPT Web **last** complements abuse-block (4h), pacing (4 rpm), and
  warmup cache — fewer autodefault hits means fewer upstream risk blocks.
- When ChatGPT Web **is** reached as last resort, stabilization behavior is
  unchanged; operators should still treat it as one browser tab, not a pool.

## Migration Plan

1. `cost-class` field + derivation in credential parser.
2. Refactor `rank.rs` + `autodefault_provider_order()`; wire cost-class into
   `effective_budget_rank` (not order list alone).
3. Reorder `gpt-5.4-nano` (and audit `gpt-5.4-mini`).
4. CLI banner + docs.
5. Tests (include beta.16 interaction cases from Risks above).
6. Bump **`0.3.0-beta.16` → `0.3.0-beta.17`** when CI green.
7. Backfill **`CHANGELOG.md` beta.12–17** (see tasks §7).

## Resolved questions (was open)

| Question | Decision | Rationale |
| --- | --- | --- |
| DeepSeek vs Gemini free | Gemini free **before** DeepSeek Web | API multi-slot + tooling; DeepSeek web rejects tools |
| github-models / opencode subsidized? | Both **`free`** in v1 | GitHub Models is explicitly free-tier; extra band adds complexity |
| ChatGPT Web kill-switch? | **No** in v1 | Last-resort ordering + session gating is enough |
| Default model | **`openai/gpt-5.4-nano`** | Matches catalog nano slug with provider prefix |
