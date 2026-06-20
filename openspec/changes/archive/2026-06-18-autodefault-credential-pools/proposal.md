## Why

Stage autodefault receives client requests for `openai/gpt-5-mini` with strict
`json_schema`, but three gaps limit throughput under 6+ parallel analyses:

1. **Model-binding drift** — `gpt-5-mini` lacks `github-models` in mapping while
   `gpt-5.4-nano` maps Groq to `llama-3.1-8b-instant` (no json_schema), so a
   blind switch to nano would drop live Groq failover.
2. **Gemini pool ceiling** — embedded catalog stops at `gemini-free-8`; operators
   want up to **16** free AI Studio projects for round-robin and sibling failover.
3. **DeepSeek Web single session** — one `deepseek-web-default` slot caps free
   web failover at `concurrent:1`; a second session doubles parallel json_schema
   capacity on the fat-payload path.

ChatGPT Web remains **one** session (paid-browser last resort) — out of scope.

## What Changes

- **Model-binding audit and alignment** for `gpt-5-mini`, `gpt-5.4-nano`, and
  `gpt-5.4-mini`:
  - Add `github-models/openai/gpt-4o-mini` to `gpt-5-mini` mapping (after free
    OpenRouter entries, before Groq scout).
  - Align nano/mini Groq targets to `groq/meta-llama/llama-4-scout-17b-16e-instruct`
    (json_schema-capable per `router/capability/providers.rs`).
  - Document parity rules so nano and mini free-tier tails stay in sync except
    where capability helpers require different Groq slugs.
- **Gemini free pool expansion** from 8 to **16** slots (`gemini-free` …
  `gemini-free-16`) with unchanged per-slot cooldown, round-robin, and sibling
  failover semantics.
- **DeepSeek Web session pool** — add `deepseek-web-2` credential slot; reuse
  existing round-robin and per-session pacing (no ChatGPT-style multi-session).
- **Verification** — routing_load / unit tests for 16-slot Gemini rotation,
  2-session DeepSeek pacing isolation, and json_schema mapping regression for
  mini vs nano.

## Capabilities

### New Capabilities

_None — enrich existing living specs to avoid duplicate capability docs._

### Modified Capabilities

- `autodefault-routing-priority`: nano/mini binding parity, GitHub on mini,
  Groq scout alignment for structured output.
- `curated-free-providers-expansion`: cost-first mapping audit requirement across
  `gpt-5-mini` and `gpt-5.4-nano` / `gpt-5.4-mini`.
- `gemini-free-multi-account`: extend free slot catalog from 8 to 16; update
  docs and round-robin scenarios.
- `deepseek-web-provider`: two-session credential pool with isolated pacing gates.

## Impact

- `ai-gateway/config/embedded/credentials.yaml` — `gemini-free-9`…`16`,
  `deepseek-web-2`
- `ai-gateway/config/embedded/model-mapping.yaml` — mini/nano/mini Groq + GitHub
- `docs/credentials.md`, `docs/providers.md`, `dev/secrets.*.yaml` examples
- `router/budget_aware/credential_balance.rs` tests — 16-slot rotation
- `router/pacing/scope.rs` — deepseek multi-session (already patterned in tests)
- `routing_load` — optional concurrent scenarios for 2 DeepSeek sessions
- **Clients (ops, not gateway code):** callers may keep reporting `gpt-5-mini`;
  switching to nano becomes safe after Groq mapping fix
