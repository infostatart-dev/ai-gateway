## Context

**Verified (2026-06-19, curl on free Ollama Cloud key):**

| Mechanism | `gpt-oss:120b` result |
|-----------|------------------------|
| `response_format: json_schema` (native API) | ❌ markdown prose |
| Schema **only** in system prompt (no `response_format`) | ✅ valid JSON 5/5 runs |
| Reflection path | Not needed in sample (call 1 always OK) |

Official docs: [Ollama structured outputs](https://docs.ollama.com/capabilities/structured-outputs)
— supported locally; **"Ollama's Cloud currently does not support structured outputs."**

Gateway today:

- `ollama-cloud` uses `OpenAICompatibleConverter` → passes `response_format` upstream.
- `ollama_cloud()` sets `supports_json_schema = false` → excluded from structured routing.
- `provider-limits.yaml`: `models: all` shared bucket, no `quota-profile: per-model`.
- Catalog lists 22 models; free key only reaches 8; Pro slugs return 403 subscription.

**Existing patterns to reuse:**

- `chatgpt_json_schema.rs` — inject schema into system message.
- `deepseek-web-structured-output` — bounded reflection retries + validation.
- `per-model-quota-domain` — `quota-profile: per-model`, Model-scope 402/429.

**Ollama quota observation (user session):**

47 requests → 15.2% session usage ⇒ ~3.09 weighted units per 1% ⇒ ~309 units/hour
session budget. Implies **weighted quota**, not raw request count:

```
usage_points = Σ (model_weight × request_count)
% = usage_points / session_budget   # session ~300–400, weekly ~800–1000
```

Indicative weights (operator model, not exact Ollama internals):

| Model class | Weight (est.) |
|-------------|---------------|
| small / fast (`gpt-oss:20b`) | 1 |
| medium | 2–3 |
| large (`gpt-oss:120b`) | 4–8 |
| heavy reasoning | 5–10 |

## Goals / Non-Goals

**Goals:**

1. **`json-schema-delivery: prompt`** path for Ollama Cloud free models with inject +
   validate + one reflection + 24h cooldown on double failure.
2. **`json-schema-delivery: native`** unchanged; prompt tier ranked lower in
   structured-output sort.
3. **`quota-profile: per-model`** for `ollama-cloud`; 403 subscription → Model scope.
4. Free catalog: **`gpt-oss:120b`** (fast-thinking) + **`gpt-oss:20b`** (fast) as
   primary free ladder; other verified-free slugs optional deprioritized band.
5. Routing-load scenarios covering positive, reflection recovery, and cooldown negative.
6. Document weighted-quota assumptions in limits notes.

**Non-Goals:**

- Emulating Ollama's exact internal weight table in pacing math (no official API).
- Pro-tier purchase / subscription automation.
- Local Ollama (`InferenceProvider::Ollama`) — separate provider; may share mapper code.
- Streaming structured output on prompt-json path (non-streaming first).

## Decisions

### D1 — `json-schema-delivery` replaces boolean-only `supports-json-schema`

Catalog field (YAML + runtime capability):

```yaml
json-schema-delivery: native | prompt | none   # default: none
```

Runtime mapping:

| Delivery | `supports_json_schema` (routing) | `json_schema_rank` default |
|----------|----------------------------------|----------------------------|
| `native` | true | 1 (best) |
| `prompt` | true | 2 (below native) |
| `none`   | false | — |

**Rejected:** New boolean `supports_json_schema_on_prompt` — duplicates delivery enum.

### D2 — Two universal upstream behaviors (shared module)

Extract / generalize from `chatgpt_json_schema.rs`:

1. **`inject_json_schema_to_system_prompt`** — build instruction from
   `response_format`, prepend/append to system message, **strip** `response_format`
   before upstream dispatch (Ollama Cloud ignores it harmfully).
2. **`reflect_on_json_validation_failure`** — on validation failure after first
   response, append assistant turn + corrective user message, **exactly one**
   reflection attempt (no second reflection; DeepSeek Web's 2-retry pattern does
   not apply here).

Apply via converter hook list keyed by `json-schema-delivery: prompt` providers
(ollama-cloud, chatgpt-web, deepseek-web already partial).

### D3 — JSON validation cooldown (24h, Model scope)

After **initial + exactly one reflection** both fail schema validation:

- Record `(credential_id, wire_slug)` in a **dedicated JSON-validation cooldown
  registry** — separate from upstream exhaustion / 404 model cooldown (different
  business failure: structured-output non-compliance, not NOT_FOUND).
- Duration: **24h** (catalog override `json-validation-failure-cooldown: 86400`).
- Scope: **Model** — other models on same credential remain eligible.
- Emit trace/metric: `json_schema_prompt_exhausted`.

**Rejected:** Sharing the 404 model cooldown store — conflates catalog phantom slugs
with prompt-json compliance failures and breaks independent statistics.

### D4 — Structured-output rank (extends D5 from per-model-quota-domain)

Within same `budget_rank` and intent tier:

1. `json_schema_rank` (native=1, prompt=2)
2. `ladder_rank`
3. `intent_proximity`

Prompt-json Ollama MUST NOT sort ahead of native-json OpenRouter/LongCat at same band.

### D5 — Ollama Cloud `quota-profile: per-model`

```yaml
ollama-cloud:
  quota-profile: per-model
```

Activates: `PacingScope::CredentialModel`, ladder filter, per-model cooldown map.

**403 subscription body** (Pro slug on free key):

- `ExhaustionScope::Model` (OmniRoute #3027 pattern)
- Long model cooldown (~24h or until weekly reset — start with 24h)
- Credential **not** marked failed

### D6 — Free catalog trim

**Only verified free autodefault slugs:**

| Slug | intent-tier | json-schema-delivery | notes |
|------|-------------|----------------------|-------|
| `gpt-oss:120b` | fast-thinking | prompt | primary free thinking |
| `gpt-oss:20b` | fast | prompt | lighter weight; not reasoning |

**Remove from embedded catalog** any slug that is unverified, ambiguous, or failed
live checks — including **`glm-4.7`** (identity/behavior unclear to operators;
removed rather than deprioritized). Document removals in English in
`provider-limits.yaml` notes: *"Removed unverified slugs (e.g. glm-4.7): upstream
identity unclear; not shipped in embedded catalog."*

Pro slugs may remain for explicit routing; free-key 403 → Model scope handles lockout.

**Rejected:** Deprioritized band for unknown slugs — adds noise without verified value.

### D7 — Weighted quota: document, do not simulate

Add `provider-limits.yaml` notes:

- Session bucket ≈ 300–400 weighted units / ~5h window
- Weekly bucket ≈ 800–1000 weighted units / 7d rolling
- Large models consume more units per call
- Gateway pacing continues TPD/RPM estimates; weights are **operator guidance** for
  ladder ordering (prefer `gpt-oss:20b` before `gpt-oss:120b` on fast intent)

Future: optional `model_weight` in limits YAML if telemetry confirms table.

### D8 — Test architecture

| Scenario | Type | Assert |
|----------|------|--------|
| Ollama prompt-json call 1 valid | Positive | 200, schema OK, no reflection |
| Ollama call 1 invalid → call 2 valid | Positive reflection | emulator returns prose then JSON |
| Ollama call 1+2 invalid | Negative cooldown | model in 24h cooldown; failover to next |
| Ollama 403 on Pro slug | Per-model block | Model scope; `gpt-oss:120b` still OK |
| Structured rank | Ordering | native OpenRouter before prompt Ollama |

Harness: extend `routing_load` + upstream-emulator wire for ollama-cloud prompt mode.

## Risks / Trade-offs

| Risk | Mitigation |
|------|------------|
| Prompt-json flaky under load | Reflection + cooldown; rank below native |
| Double upstream call doubles latency/cost | Only on validation failure; 120b first-call success was 5/5 |
| 24h cooldown too aggressive | Catalog-tunable duration; metric before tuning |
| Weight table guess wrong | Notes only; revisit with dashboard data |
| Overlap with `per-model-quota-domain` | Apply together; complete PMQ task 30 first |

## Migration Plan

1. Ship catalog + capability changes (delivery enum, ollama trim).
2. Ship mapper inject + strip `response_format` for prompt delivery.
3. Ship reflection executor + JSON cooldown registry.
4. Enable `quota-profile: per-model` + 403 Model scope.
5. Update ladders; run routing-load scenarios.
6. Rollback: revert `json-schema-delivery` to `none` on ollama-cloud slugs.

## Decisions (resolved)

| Topic | Decision |
|-------|----------|
| `glm-4.7` | **Remove** from embedded catalog; document in English that upstream identity is unclear |
| Reflection retries | **Exactly 1** per client request; no second reflection |
| JSON-validation cooldown store | **Separate registry** from 404/upstream exhaustion cooldowns (distinct business error + metrics) |
