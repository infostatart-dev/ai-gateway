## Why

`autodefault` currently ranks browser sessions (`chatgpt-web`, `deepseek-web`)
ahead of free API keys, so a ~$20/mo ChatGPT Plus session absorbs traffic before
OpenRouter, Gemini free, and other $0 paths. The default nano model
(`gpt-5.4-nano`) maps to paid fallbacks first while `gpt-5-mini` maps correctly.

2026 gateway practice and reference auto-router design agree: **cascade by cost**
(free API first), **fallback by reliability** (paid API, then paid browser last).
This change aligns provider order, credential cost-class, and model-binding YAML
with that policy.

Release target: **`0.3.0-beta.17`** (after `payload-aware-routing` beta.16).

**Prerequisite landed (2026-06-16):** beta.16 code is in tree; workspace version is
already **`0.3.0-beta.16`**. Beta.17 implementation and version bump are still
pending (CI fix in parallel). `CHANGELOG.md` lags at beta.11 — backfill
beta.12–17 is part of the release tasks (see `design.md`, tasks §7).

## What Changes

- Add `cost-class` (`free` | `paid` | `paid-browser`) to credential slots;
  derive from `tier` when omitted.
- Reorder autodefault: free API → Gemini free → DeepSeek Web → paid API →
  ChatGPT Web **last**.
- Sort budget-aware candidates: cost-class → budget-rank → provider priority.
- Reorder `gpt-5.4-nano` in `model-mapping.yaml` (free-first, mirror `gpt-5-mini`).
- CLI/docs default model: **`openai/gpt-5.4-nano`**.

## Capabilities

### New Capabilities

- `autodefault-routing-priority`: Cost-class-first ordering, provider priority
  rebalance, model-binding alignment, default-model convention.

### Modified Capabilities

- None.

## Impact

- `config/read.rs`, `router/budget_aware/rank.rs`, `credentials.yaml`,
  `model-mapping.yaml`, `cli/helpers.rs`, `docs/routing.md`
- Workspace version: **`0.3.0-beta.17`**
