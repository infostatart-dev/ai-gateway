## Context

The gateway already routes autodefault traffic across a curated free stack:
`opencode`, `openrouter`, `github-models`, `mistral`, `groq`, `cerebras`,
`cloudflare`, multi-slot `gemini`, and `deepseek-web`. Each addition follows
the same pattern established by `github-models`: embedded `providers.yaml` entry,
credential slot, conservative `model-capabilities`, `provider-limits.yaml` tier,
optional `providers.rs` capability helper, autodefault gating on credential
presence, and mock-backed tests.

Eight additional OpenAI-compatible providers offer documented recurring free
quotas with API keys. Groq's developer tier is free but misclassified as paid.
OpenRouter's live catalog exposes more `:free` variants and an `openrouter/free`
router slug than the embedded list currently registers.

## Goals / Non-Goals

**Goals:**

- Ship Tier 1 providers (`longcat`, `doubao`, `ollama-cloud`, `inclusionai`,
  `sambanova`, `bluesminds`, `bazaarlink`, `cohere`) as first-class
  OpenAI-compatible providers with curated catalogs and free credential slots.
- Reclassify `groq-default` to `cost-class: free`.
- Expand OpenRouter Tier 2 (`:free` slugs + `openrouter/free`) and wire them into
  autodefault mappings.
- Extend autodefault provider priority to reflect documented free-token budgets
  and JSON-schema suitability.
- Document conservative rate limits; fail open on unknown TPM at payload filter
  time (existing behavior).

**Non-Goals:**

- Browser/web session providers (Tier 3+): `duckduckgo-web`, `qwen-web`,
  `t3-web`, `perplexity-web`, etc.
- Rate-limit-only backends without published token caps: `tencent`, `siliconflow`,
  `nvidia` NIM direct, `baidu`, `publicai`, `sparkdesk`.
- Paid-tier expansion, embeddings routes, or multi-account pools beyond one
  default slot per new provider in v1.
- Live catalog sync jobs; slugs are curated statically and verified against
  OpenRouter's public models API at implementation time.

## Decisions

### 1. Single delta spec, one capability id

**Decision:** One spec file `specs/curated-free-providers-expansion/spec.md`
covers all providers, Groq reclassification, OpenRouter Tier 2, autodefault order,
limits, mapping, and tests. Autodefault requirement changes are expressed as
ADDED requirements inside that spec rather than a separate delta file.

**Rationale:** User requested one consolidated specification; avoids fragmented
review across nine provider-specific specs.

**Alternative considered:** Per-provider specs (`longcat-provider`, etc.) —
rejected as harder to review holistically.

### 2. OpenAI-compatible dispatcher only

**Decision:** All Tier 1 providers use the existing OpenAI-compatible client
path (`InferenceProvider::Named` + standard bearer auth). No custom executors.

**Rationale:** Every target publishes `/v1/chat/completions` (Cohere via
`/compatibility/v1`). Matches `github-models` and `cerebras` integration cost.

**Exception:** `ollama-cloud` uses `https://ollama.com/v1/` — distinct from
local `ollama` (`localhost:11434`) to avoid operator confusion.

### 3. Curated static catalogs over live discovery

**Decision:** Model lists are hand-curated in `providers.yaml`. OpenRouter
`:free` slugs are refreshed once during implementation from
`https://openrouter.ai/api/v1/models` (filter `pricing.prompt == "0"` or
`:free` suffix).

**Rationale:** Gateway already uses static catalogs; live sync is out of scope.

### 4. Autodefault priority by free-token budget, then schema fit

**Decision:** Insert new providers into `autodefault_provider_order()` as follows
(when credential + catalog present):

| Rank | Provider | Rationale |
| ---: | --- | --- |
| 0 | `opencode` | Existing free JSON-schema leader |
| 1 | `longcat` | Largest documented daily free pool (Flash-Lite) |
| 2 | `mistral` | ~1B tokens/month experiment tier |
| 3 | `openrouter` | Aggregator + `:free` |
| 4 | `github-models` | High-quality free chat catalog |
| 5 | `bazaarlink` | `auto:free` aggregator |
| 6 | `bluesminds` | Free aggregator, moderate pool |
| 7 | `groq` | Reclassified free; fast inference |
| 8 | `cerebras` | Existing |
| 9 | `cloudflare` | Existing |
| 10 | `sambanova` | Documented free tier |
| 11 | `inclusionai` | Daily free pool |
| 12 | `ollama-cloud` | Cloud free tier |
| 13 | `cohere` | Small trial quota; late free band |
| 14 | `doubao` | Regional; late free band |
| 15 | `gemini` | Multi-slot free |
| 16 | `deepseek-web` | Browser session |
| 17+ | `anthropic`, `openai`, `chatgpt-web` | Paid / browser last |

Within the same rank, existing `budget-rank` and cost-class rules apply.

**Alternative considered:** Alphabetical insertion — rejected; ignores token
economics.

### 5. Groq tier rename in limits YAML

**Decision:** Add `groq.tiers.free` mirroring current `developer` limits; credential
slot uses `tier: free`. Keep `developer` alias or migrate entries — implementer
maps credential `tier: free` to limits key `free`.

**Rationale:** Aligns cost-class derivation with other free API providers.

### 6. OpenRouter Tier 2 scope

**Decision:** Add at minimum:

- Router slug `openrouter/free` (upstream id `openrouter/free`).
- Additional `:free` slugs verified live, including:
  `arcee-ai/trinity-large-preview:free`,
  `arcee-ai/trinity-mini:free`,
  `deepseek/deepseek-r1:free`,
  `nvidia/nemotron-3-nano-30b-a3b:free` (if live),
  `openrouter/sonoma-dusk-alpha:free` and siblings when listed,
  plus any other zero-cost slugs returned by the models API at implementation
  time that support chat completions.

**Rationale:** Tier 2 is catalog-only on existing provider — lowest integration
cost, highest model diversity.

### 7. Capability metadata: conservative defaults

**Decision:** Per-model `supports-tools`, `supports-json-schema`, `context-window`,
`reasoning` set explicitly in YAML. Fallback `providers.rs` named helpers only
where YAML is absent (same as `cerebras` / `opencode`).

**Rationale:** Free tiers change; under-advertising beats false positives in
structured-output routing.

## Risks / Trade-offs

- **[Risk] Free tiers change or disappear** → Mitigation: `observed-at` notes in
  `provider-limits.yaml`; conservative RPM/RPD; failover via autodefault.
- **[Risk] Aggregators (`bazaarlink`, `bluesminds`) proxy third-party models**
  → Mitigation: document ToS caution; treat as `cost-class: free` only; no
  production SLA claims.
- **[Risk] `doubao` requires China-region Volcengine account** → Mitigation: late
  autodefault rank; optional slot — omitted when credential unset.
- **[Risk] OpenRouter `:free` slug rot** → Mitigation: verify at implement time;
  document that missing slugs yield mapping skip, not startup failure.
- **[Risk] Large single change** → Mitigation: tasks.md phases per provider;
  shared test harness pattern.

## Migration Plan

1. Land YAML + credentials + limits (no behavior change until keys set).
2. Update `read.rs` autodefault order and Groq credential tier.
3. Extend capability helpers and model mapping.
4. Add tests; bump version to `0.3.0-beta.20`.
5. Operators add `AI_GATEWAY_CREDENTIAL_*` vars incrementally — missing slots
   skipped at startup (existing pattern).

Rollback: revert commit; unset new env vars. No data migration.

## Open Questions

- Whether `longcat` public beta adds `LongCat-2.0-Preview` to the curated list
  at implementation time (include if live and documented).
- Exact live set of OpenRouter `:free` slugs — resolved during `/opsx:apply` via
  models API probe, not at spec time.
