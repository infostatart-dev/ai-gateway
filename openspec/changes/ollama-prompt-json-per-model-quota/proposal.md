## Why

Ollama Cloud **does not support native structured outputs** (`response_format` /
`format` API) per official docs, yet the same models **do** return valid JSON when
the schema is injected into the system prompt (verified 2026-06-19 on free tier).
Today the gateway forwards `response_format` unchanged → markdown prose and wasted
quota. In parallel, Ollama Cloud needs **per-model** blocking (403 subscription on
Pro slugs must not kill the whole credential) and a **trimmed free catalog** focused
on `gpt-oss:120b` / `gpt-oss:20b`, with pacing notes reflecting Ollama's **weighted**
session/weekly buckets—not a flat request counter.

## What Changes

- Introduce **`json-schema-delivery`** catalog capability with two delivery classes:
  **`native`** (upstream API enforces schema) and **`prompt`** (gateway injects
  schema into system prompt, strips `response_format` upstream).
- Add universal upstream behaviors (reusable beyond Ollama):
  - **System prompt schema injection** (extend existing `chatgpt_json_schema` path).
  - **Reflection retry** on failed JSON/schema validation (one corrective turn).
  - **Model-level JSON cooldown** (24h) when reflection also fails.
- Rank **`prompt`** delivery **below** **`native`** in structured-output ordering
  (`json_schema_rank` bands), without excluding prompt-capable models from the pool.
- Enable **`quota-profile: per-model`** for `ollama-cloud`; classify **403
  subscription** as **`Model`** scope (per-slug lockout, credential stays active).
- Trim embedded **free Ollama Cloud catalog** to **`gpt-oss:120b`** and **`gpt-oss:20b`**
  only; remove unverified/unclear slugs (e.g. **`glm-4.7`** — upstream identity unclear,
  documented in English in limits notes)
- Document Ollama Cloud **weighted quota model** in `provider-limits.yaml` notes
  (session ≈ 300–400 weighted units/hour; large models cost more units per call).
- Routing-load / integration scenarios: positive prompt-json success, reflection
  recovery, and negative double-failure → 24h model cooldown.

## Capabilities

### New Capabilities

- `prompt-json-schema-upstream`: Universal prompt-injection, reflection retry, and
  JSON-validation cooldown for upstreams with `json-schema-delivery: prompt`.
- `ollama-cloud-per-model-quota`: Ollama Cloud per-model pacing, 403 subscription
  model lockout, free-catalog trim, weighted-quota operator notes.

### Modified Capabilities

- `structured-output-routing`: Order candidates by `json-schema-delivery` tier
  (native before prompt) within budget rank.
- `per-model-exhaustion-scopes`: Add 403 subscription-required → `Model` scope on
  `per-model` providers.
- `provider-model-catalog`: Add `json-schema-delivery` field; Ollama Cloud free-tier
  model entries and verification metadata.

## Impact

- **Config**: `providers.yaml`, `provider-limits.yaml`, `provider-ladders.yaml`
  (ollama-cloud free ladder).
- **Mapper**: `OpenAICompatibleConverter` or dedicated ollama-cloud converter;
  shared `chatgpt_json_schema` + new reflection executor.
- **Router**: `capability/providers.rs`, structured-output rank, **dedicated**
  JSON-validation cooldown registry (not shared with 404 exhaustion store).
- **Tests**: routing-load scenarios (prompt-json positive, reflection, cooldown
  negative); extend `per-model-exhaustion-scopes` harness for Ollama 403.
- **Docs**: `docs/providers.md` — Ollama Cloud native vs prompt JSON, weighted quota.
- **Depends on**: `per-model-quota-domain` (29/30 tasks) — finish remaining task or
  merge overlapping work during apply.
