## 0. Reusable abstractions (before Gemini wiring)

- [x] 0.1 Add `ProviderQuotaProfile` (`per-model` | `per-slot` | `per-session`) to provider catalog metadata; set `gemini: per-model`, session providers unchanged
- [x] 0.2 Extract shared `catalog_limit_resolve(provider, tier, request_model)` from emulator `candidate_slugs` logic into `ai-gateway` (public or `pub(crate)`); emulator calls shared helper
- [x] 0.3 Add `PacingScope` enum (`Session` | `Credential` | `CredentialModel`) in `router/pacing/scope.rs`; migrate `gate_scope_key` to return scope tuple
- [x] 0.4 Add `ExhaustionScope` (`Model` | `Slot` | `Project`) + `classify_exhaustion_scope(response)` in `router/retry_after/`
- [x] 0.5 Add `ModelLadderRegistry` + embedded `provider-ladders.yaml` schema (provider → tier → bands → model list)
- [x] 0.6 Unit test: gateway and emulator resolve identical limits for fixture slug set (sync contract)

## 1. Free-tier limit catalog and routable models

- [x] 1.1 Update `provider-limits.yaml` Gemini `free` tier per-model RPM/TPM/RPD to match AI Studio snapshot (3-flash/3.5/2.5 RPD 20, 3.1-flash-lite RPD 500); bump `observed-at`
- [x] 1.2 Add `gemini-3.5-flash-preview`, `gemini-3.1-flash-lite` to `providers.yaml` with capabilities and intent_tier metadata
- [x] 1.3 Populate `provider-ladders.yaml` entry `gemini.free` (fast / capacity / stability bands)
- [x] 1.4 Unit tests: every ladder slug resolves via `catalog_limit_resolve` to a catalog model entry

## 2. Per-model pacing (catalog-quota-pacing)

- [x] 2.1 Add `PacingLimits::resolve_for_model(provider, tier, model)` via shared `catalog_limit_resolve`
- [x] 2.2 Extend `PacingRegistry::gate_for(provider, credential, model)` — `CredentialModel` scope when `ProviderQuotaProfile::PerModel`
- [x] 2.3 Pass upstream model from dispatcher into `acquire_upstream_pacing` (`dispatch.rs`; session providers pass `None` for model)
- [x] 2.4 Unit tests: same credential, two models → distinct gates; RPD on model A does not block model B
- [x] 2.5 Unit tests: `per-slot` / `per-session` providers unchanged (no model suffix on gate key)

## 3. Per-model failure scope and sibling skip

- [x] 3.1 Introduce `FailedModelKey(credential_id, model)` in failover walk; keep `failed_credentials` for `ExhaustionScope::Slot` / `::Project`
- [x] 3.2 Wire `classify_exhaustion_scope`; narrow `skip_free_siblings_on_exhaustion` to `ExhaustionScope::Project` only
- [x] 3.3 Cooldown state keyed by `(credential_id, model)` for `ExhaustionScope::Model`; credential-level for slot/project
- [x] 3.4 Unit tests: model RPM 429 does not insert whole credential into `failed_credentials`
- [x] 3.5 Unit tests: project billing 429 skips free siblings; per-model RPD does not

## 4. Intra-slot model ladder and stability band

- [x] 4.1 Rank candidates on same credential via `ModelLadderRegistry::band_index` before round-robin offset (Gemini free first consumer)
- [x] 4.2 Wire stability band as last hop on same credential (after fast+capacity exhausted or gated)
- [x] 4.3 Respect json_schema and intent floor when selecting capacity/stability models
- [x] 4.4 Route trace: `quota_scope`, `model_ladder_band`, `model_ladder_position` (generic keys)
- [x] 4.5 Unit tests: band order fast → capacity → stability; stability not first hop when fast eligible
- [x] 4.6 Unit test: provider without ladder entry keeps existing rank order (groq smoke)

## 5. Emulator and routing_load tests

- [x] 5.1 Upstream-emulator: per-model limit buckets via shared resolve; distinct 429 bodies (`model-rpd` vs `project-billing`)
- [x] 5.2 `routing_load/scenarios/gemini_model_ladder_same_slot.rs` — 3-flash exhausted → 3.1-lite same credential succeeds
- [x] 5.3 `routing_load/scenarios/gemini_stability_escalation.rs` — fast band out → 2.5-pro same slot before inter-slot hop
- [x] 5.4 Regression: `gemini_sixteen_slot`, `failover_rpm` still pass (inter-slot behavior unchanged)
- [x] 5.5 Register scenarios in `routing_load.rs` integration test binary
- [x] 5.6 Unit/integration: `catalog_limit_resolve` parity test (gateway vs emulator fixture table)

## 6. Docs, OpenSpec, release

- [x] 6.1 Update `docs/providers.md` and `docs/credentials.md` — quota scope hierarchy (tier/slot/model), ladder bands, stability escalation
- [x] 6.2 Document extension checklist for adding per-model ladder to another provider
- [x] 6.3 `mise exec -- openspec validate gemini-per-model-quota-ladder --strict`
- [x] 6.4 `mise run predeploy:rust` on touched modules
- [x] 6.5 `CHANGELOG.md` entry under next beta (after implementation)
