## Why

Autodefault treats the client `model` field as a routing key into
`model-mapping.yaml`, but business clients use model names as **intent signals**
(fast vs deep thinking vs stability), not as upstream SKUs. They do not know or
care whether `gpt-5-nano` maps to `llama-4-scout` or `gemini-2.0-flash`.

Today this causes three failures:

1. **Binding drift** — `gpt-5-mini`, `gpt-5.4-nano`, and `gpt-5.4-mini` need
   near-identical YAML lists maintained by hand; any drift drops capable
   failover paths (e.g. Groq scout vs `llama-3.1-8b-instant`).
2. **Wrong intent inference** — `reasoning_preferred_for_model_name` marks
   `gpt-5-nano`, `gpt-5-mini`, and plain `gpt-5` alike as reasoning-preferred,
   so a nano request ranks reasoning models first and, after cooldown failover,
   lands on the largest available model — the opposite of what the client asked.
3. **Narrow candidate pool** — `matches_source_model` filters the global
   credential×model pool down to one alias's mapping before cost-class ranking,
   wasting Gemini multi-slot rotation and other cross-provider capacity.

Autodefault should interpret client model names as **intent** (latency tier,
reasoning depth, stability floor), select from the full capable pool, prefer
fast/cheap first, and **escalate upward** when free paths fail — never
**de-escalate** to a smaller model when the client asked for quality or
stability.

## What Changes

- Add **intent-based routing** for router `autodefault` (default): derive
  `RoutingIntent` from source model name + payload (json_schema, tools, context).
- Replace per-alias **strict model binding** in autodefault with **pool selection**:
  filter candidates by hard requirements + intent floor, not by
  `model-mapping.yaml` key match.
- Introduce **asymmetric stability policy**:
  - **Escalate up** allowed — nano/min client may receive mini/standard/deep
    upstream when cheaper candidates are in cooldown or exhausted (stability).
  - **De-escalate down** forbidden — deep/standard client must never receive a
    fast-tier upstream just because it is free and available.
- Fix **intent tier inference** — `gpt-5-nano`/`mini` → **fast-thinking** (same
  intent); plain `gpt-5` / `o1` / `o3` → **deep**; remove substring `gpt-5`
  lumping in `reasoning_preferred_for_model_name`.
- **Payload shape rules:** json strict → json_schema-capable pool only; plain →
  json and non-json upstream within intent band.
- Document and test **four-case acceptance matrix** (mini/nano × plain/json strict).
- Add router config knob `source-model-selection: strict | intent` (autodefault
  → `intent`; human-configured routers → `strict` default unchanged).
- Annotate upstream catalog models with **intent tier metadata** (fast /
  standard / deep) for rank and guard logic; mapping.yaml remains for strict
  mode and documentation, not autodefault gate.
- Add observability: response headers / route trace expose resolved intent tier
  and selected upstream model.
- Tests and routing_load scenarios for the **four-case acceptance matrix**
  (mini/nano × plain/json strict), deep-no-downgrade, and stability escalation.

## Capabilities

### New Capabilities

- `autodefault-intent-routing`: intent extraction, pool selection, asymmetric
  stability (escalate-up / no downgrade), router selection mode config.

### Modified Capabilities

- `autodefault-routing-priority`: selection no longer depends on per-alias
  mapping order; cost-class ranking applies to intent-filtered pool.
- `payload-aware-routing`: payload filter runs on intent pool; best-effort tail
  respects intent floor (no downgrade on oversized requests).
- `structured-output-routing`: json_schema hard constraint unchanged; intent
  pool widens capable free targets without alias sync.

## Impact

- `ai-gateway/src/router/capability/mod.rs` — intent types, fix
  `reasoning_preferred_for_model_name`, `capability_fit_score` axes
- `ai-gateway/src/router/budget_aware/selection.rs`,
  `selection_mode.rs`, `payload.rs` — pool filter replaces
  `matches_source_model` when `intent` mode
- `ai-gateway/src/config/router.rs`, `read.rs` — autodefault router config
- `ai-gateway/src/config/model_capability.rs` or new `intent_tier` catalog —
  upstream tier metadata
- `ai-gateway/config/embedded/model-mapping.yaml` — role narrows to strict-mode
  routers; autodefault parity maintenance reduced
- `docs/routing.md`, `openspec/specs/autodefault-routing-priority/spec.md`
- `routing_load` scenarios — intent ordering and stability escalation
- **Relationship to `autodefault-credential-pools`**: mapping parity tasks (D2–D6)
  become optional once intent pool lands; pool expansion (Gemini×16, DeepSeek×2)
  remains valuable and composes with intent routing.
