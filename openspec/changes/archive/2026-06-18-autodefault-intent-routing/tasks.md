## 1. Intent model and config

- [x] 1.1 Add `router/intent/` module with `IntentTier`, `RoutingIntent`, and `extract_routing_intent(source_model, requirements)` using longest-match-first name table (D1)
- [x] 1.2 Add `SourceModelSelection` enum (`strict` | `intent`) to `RouterConfig` in `config/router.rs` with serde default `strict`
- [x] 1.3 Set `source_model_selection: Intent` in `build_autodefault_router_config` (`config/read.rs`)
- [x] 1.4 Thread selection mode through `BudgetAwareRouter::new_*` in `budget_aware/factory.rs` and store on router struct

## 2. Upstream intent tier metadata

- [x] 2.1 Add `intent_tier: IntentTier` to `ModelCapability` (`router/capability/mod.rs`)
- [x] 2.2 Implement `default_intent_tier(provider, model_slug)` helper (parallel to json_schema hints in `capability/providers.rs`)
- [x] 2.3 Optional: allow `intent-tier` override per model in `providers.yaml`; wire through `get_model_capability`
- [x] 2.4 Unit tests: scout/gpt-oss → FastThinking, dumb flash → Fast, sonnet/o1 → Deep

## 3. Fix reasoning misclassification (standalone, ship early)

- [x] 3.1 Rewrite `reasoning_preferred_for_model_name` — remove bare `gpt-5` / nano / mini lumping; deep keywords only when not fast-classified (D5)
- [x] 3.2 Replace reasoning bool boost in `capability_fit_score` with `intent_proximity_score(preferred_tier, candidate.intent_tier)`
- [x] 3.3 Update `router/capability/tests.rs`: mini/nano → fast-thinking (not deep); plain gpt-5 → deep; json strict vs plain pool width

## 4. Intent pool selection

- [x] 4.1 Add `matches_intent(candidate, intent)` replacing binding check when mode is `intent`
- [x] 4.2 Branch `selection_mode.rs` and `selection.rs` `ordered_candidates` filters: strict → `matches_source_model`, intent → intent floor + capability/payload
- [x] 4.3 Mirror intent branch in `capability/mod.rs` `ordered_candidates` for capability-aware routers using same config knob
- [x] 4.4 Keep web providers (`chatgpt-web`, `deepseek-web`) permissive in both modes

## 5. Preferred-tier-first rank and stability escalation

- [x] 5.1 Segment candidates into preferred-tier band vs escalation band in `ordered_candidates` (D6)
- [x] 5.2 Extend `failover_loop.rs` to advance from preferred band to escalation band when segment exhausted
- [x] 5.3 Add intent proximity tiebreak to rank after cost-class / budget-rank in `rank_score.rs` or dedicated intent rank helper
- [x] 5.4 Ensure deep-floor requests never include below-floor candidates in initial or escalation segments

## 6. Payload best-effort and structured output

- [x] 6.1 Update `payload.rs` best-effort tail to respect `floor_tier` in intent mode (D7)
- [x] 6.2 Ensure json_schema_rank / capability fit applies within tier band before cross-tier promotion (structured-output spec)
- [x] 6.3 Regression: mini/nano json strict does not promote deep reasoning model ahead of fast-thinking scout in preferred band

## 7. Observability and docs

- [x] 7.1 Add `X-Routing-Intent-Tier` and `X-Routing-Selection-Phase` response headers (`middleware/response_headers.rs`)
- [x] 7.2 Extend route trace in `budget_aware/trace.rs` with intent tier and phase
- [x] 7.3 Update `docs/routing.md` — intent routing, asymmetric stability, strict vs intent modes, mapping.yaml role change

## 8. Tests and routing_load — four-case acceptance matrix

- [x] 8.1 **Acceptance A:** `gpt-5-mini` + json strict → fast-thinking intent, json-only pool, first hop not deep while free fast-thinking json candidates exist
- [x] 8.2 **Acceptance B:** `gpt-5-mini` + plain → fast-thinking intent, json + non-json pool, ranks available free first
- [x] 8.3 **Acceptance C:** `gpt-5-nano` + json strict → same intent and pool rules as Acceptance A (mini ≡ nano)
- [x] 8.4 **Acceptance D:** `gpt-5-nano` + plain → same intent and pool rules as Acceptance B (mini ≡ nano)
- [x] 8.5 Unit: plain mini/nano includes upstream without json_schema; json strict excludes them
- [x] 8.6 Unit: gpt-5 deep intent excludes scout/fast-thinking below floor even when free
- [x] 8.7 Unit: fast-thinking escalation — after band exhaustion, deep-tier attempted with phase=escalated
- [x] 8.8 Unit: strict router regression — binding gate unchanged
- [x] 8.9 routing_load scenario: parallel json strict nano requests spread across fast-thinking pool without first hop on largest deep model
- [x] 8.10 Run `openspec validate autodefault-intent-routing --strict`

## 9. Cross-change coordination

- [x] 9.1 Document in change notes: `autodefault-credential-pools` mapping parity tasks (D2–D6) optional after intent pool; keep Gemini×16 / DeepSeek×2 pool expansion
- [x] 9.2 CHANGELOG entry for intent routing and reasoning misclassification fix
