## 1. Catalog schema and slug hotfix

- [x] 1.1 Add `ProviderModelEntry` (or extend providers config) with `upstream_slug`, `catalog_key`, optional `aliases`; support legacy string-only entries during migration
- [x] 1.2 Fix Gemini `providers.yaml`: `gemini-3.5-flash` (remove `-preview`); remove `gemini-1.5-*`; add `gemini-2.5-flash-lite`; set `last_verified_at` / `verify_source`
- [x] 1.3 Fix `provider-ladders.yaml` free band: `gemini-3.5-flash`, capacity + stability with `gemini-2.5-flash-lite`; remove `gemini-2.5-pro` from free tier
- [x] 1.4 Wire dispatcher/factory to use `upstream_slug` for capability.model wire id; keep catalog_key for `catalog_limit_resolve`
- [x] 1.5 Unit test: preview entry sends preview upstream, resolves `gemini-3-flash` catalog limits

## 2. Catalog verification (CI gate)

- [x] 2.1 Capture frozen fixture `ai-gateway/tests/fixtures/gemini-listmodels.json` from ListModels (generateContent models only)
- [x] 2.2 Add `scripts/verify_provider_catalog.rs` or mise task `catalog:verify-gemini` — assert providers + ladder slugs ⊆ fixture
- [x] 2.3 Wire verify into `predeploy:rust` when `providers.yaml`, `provider-ladders.yaml`, or fixture changes
- [x] 2.4 Document OpenAI-compat verify extension in `docs/providers.md`

## 3. Per-model exhaustion scopes

- [x] 3.1 Thread `ProviderQuotaProfile` into `classify_exhaustion_scope(status, body, class, profile)`
- [x] 3.2 Map per-model: 404 NOT_FOUND → Model; 400 unsupported → Model
- [x] 3.3 Add `looks_like_high_demand(body)` for Gemini 503; per-model 503 → Slot with short cooldown
- [x] 3.4 Long model cooldown (≥1h) for 404/unsupported on per-model profile in `update_failure_state_scoped`
- [x] 3.5 Unit tests: per-model 404→Model; per-model 503→Slot; per-slot 404→Slot; billing→Project (matrix from spec)

## 4. Ladder-only intra-slot walk

- [x] 4.1 Add `ladder_slugs(provider, tier)` helper on `ModelLadderRegistry`
- [x] 4.2 Filter `ordered_candidates` for per-model+ladder providers: keep only ladder slugs per credential before ranking
- [x] 4.3 Ensure paid `gemini-default` is ordered after all free ladder walks, not mixed into intra-slot steps
- [x] 4.4 Unit test: factory/selection produces only ladder slugs for Gemini free credentials

## 5. Stability and intent integration

- [x] 5.1 Confirm stability band ranks after fast/capacity in `ladder_rank.rs` with updated YAML
- [x] 5.2 Unit test: fast exhausted → `gemini-2.5-flash-lite` before inter-slot hop; never `gemini-2.5-pro` on free
- [x] 5.3 Acceptance test: fast-thinking request uses intra-slot capacity before groq when Gemini lite has quota

## 6. routing_load and emulator

- [x] 6.1 Scenario `gemini_404_retires_model_not_slot`: emulator 404 on phantom slug → next ladder model same credential succeeds
- [x] 6.2 Scenario `gemini_503_high_demand_continues_ladder`: 503 body on 3.5-flash → 3.1-flash-lite same slot
- [x] 6.3 Scenario `gemini_stability_escalates_up`: fast band exhausted → 2.5-flash-lite not 1.5/dead slug
- [x] 6.4 Emulator: add 404 not-found and 503 high-demand body templates matching Google shapes
- [x] 6.5 Register new scenarios in `routing_load/scenarios/mod.rs` and `tests/routing_load.rs`

## 7. Observability and docs

- [x] 7.1 Route trace / dispatch log: include `exhaustion_scope` and `quota_profile` on classified failures (extend existing fields if needed)
- [x] 7.2 Update `docs/providers.md` and `docs/credentials.md` with catalog verify workflow and corrected Gemini slugs
- [x] 7.3 CHANGELOG entry for `0.4.2-beta.2` (or next beta): slug fix, scope fix, ladder-only walk

## 8. Validation

- [x] 8.1 `mise run catalog:verify-gemini` passes
- [x] 8.2 `cargo test --features testing` — new unit + routing_load scenarios green
- [x] 8.3 `mise run openspec:validate provider-model-reality --strict`
- [ ] 8.4 Stage smoke: Gemini 404 rate drops; `gemini-3.1-flash-lite` success visible on exhausted 3-flash slots
