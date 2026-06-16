## Why

The gateway has strong unit and sequential integration coverage for budget-aware autodefault routing (round-robin, sibling failover, payload-aware filtering), but no concurrent load-style verification that the router behaves correctly under burst traffic with large `json_schema` payloads — without spending real tokens or exposing production API keys. Existing k6 benchmarks measure gateway throughput against a single mock provider, not autodefault credential distribution, pacing, or ChatGPT Web last-resort behavior.

## What Changes

- Introduce a unified **routing load verification** framework: shared fixtures, stats assertions, and scenario catalog — not a one-off test file.
- Add **Level 1** (router concurrent) and **Level 2** (Harness + Stubr end-to-end) tests runnable in CI on every PR, using synthetic credentials and mock upstream only.
- Assert routing correctness via **`GET /v1/observability/provider-stats`** per `(provider, credential)` and optional `X-Gateway-Provider-Usage` spot checks — not LLM response quality.
- Extend the existing **`push_test_call_response`** test hook with **per-credential mock responses** so concurrent failover scenarios are deterministic (replacing the current global FIFO queue limitation).
- Cover the prod-like profile: four free Gemini slots, optional paid `gemini-default`, ChatGPT Web as paid-browser last resort, fat `json_schema` context — parameterized `N` free Gemini slots for future expansion.
- Document how to add new routing load scenarios as the autodefault stack evolves.
- **Optional Phase 2 (out of initial scope):** k6 routing suite for nightly/release — reuses the same fixture contract, not duplicated logic.

## Capabilities

### New Capabilities

- `routing-load-verification`: Concurrent and burst routing correctness tests for budget-aware autodefault — round-robin fairness, payload-aware pre-filter under load, Gemini sibling/paid failover, ChatGPT Web last-resort invariants, pacing backpressure — without live provider keys or token spend. Includes shared test fixtures, stats assertion helpers, per-credential mock hook, CI tiering (L1 PR / L2 PR / L3 nightly optional), and contributor documentation.

### Modified Capabilities

<!-- No changes to production routing behavior — test infrastructure only. -->

## Impact

- **Tests:** new `ai-gateway/tests/routing_load/` tree; extensions to `ai-gateway/src/router/budget_aware/call.rs` test hooks; possible credential-specific Stubr stubs for Level 2.
- **Docs:** short section in `docs/` or `DEVELOPMENT.md` on adding routing load scenarios.
- **CI:** additional `cargo test` targets (serial, `testing` feature); no change to release artifact build.
- **Not affected:** production routing logic, credentials.yaml catalog size (stays at four free Gemini unless a separate change expands it), k6 throughput benchmarks (remain as-is for gateway RPS).
