## 1. Test infrastructure — per-credential mocks

- [x] 1.1 Extend `call.rs` test hooks with per-`ProviderCredentialId` response map (keep global FIFO fallback for existing tests)
- [x] 1.2 Add `clear_test_call_responses` / push helpers for credential-keyed mocks with unit tests
- [x] 1.3 Verify existing `credential_failover.rs` and `failover_integration.rs` still pass unchanged

## 2. Shared routing load framework

- [x] 2.1 Create `ai-gateway/tests/routing_load/mod.rs` with `fixture`, `assert_stats`, and `payload` modules
- [x] 2.2 Implement `RoutingLoadProfile::autodefault_prod_like(N)` — secrets builder, router config, fat `json_schema` body
- [x] 2.3 Implement stats helpers: `fetch_stats`, `attempts_for_credential`, `assert_fairness_band`, `assert_zero_attempts`
- [x] 2.4 Add deterministic payload generator targeting configurable estimated token count

## 3. Level 1 — router concurrent scenarios

- [x] 3.1 `round_robin.rs` — 32 concurrent successes, 4 free Gemini, fairness ±25%, chatgpt-web zero
- [x] 3.2 `payload_filter.rs` — concurrent fat json_schema skips TPM-limited provider (groq), gemini receives traffic
- [x] 3.3 `failover_rpm.rs` — concurrent transient 429 on `gemini-free` → sibling success, no chatgpt-web
- [x] 3.4 `failover_quota.rs` — daily quota skips free siblings → `gemini-default`, no chatgpt-web
- [x] 3.5 `chatgpt_last_resort.rs` — all free API cooldown/injection → terminal `chatgpt-web-default`
- [x] 3.6 `pacing_burst.rs` — 10 concurrent chatgpt-web with paused tokio time, concurrent:1 respected
- [x] 3.7 `shaper_backpressure.rs` — decision shaper free-tier limit vs 32 inbound, no spurious upstream

## 4. Level 2 — Harness end-to-end scenarios

- [x] 4.1 Harness scenario: concurrent round-robin via HTTP + provider-stats assertions (4 gemini stubs success)
- [x] 4.2 Harness scenario: payload-aware filter under concurrent HTTP dispatch with fat json_schema body
- [x] 4.3 Mark all routing load tests `#[serial_test::serial]` and use fresh Harness/AppState per case

## 5. Documentation and CI

- [x] 5.1 Add routing load verification section to `DEVELOPMENT.md` (fixture, levels, adding scenarios)
- [x] 5.2 Ensure `cargo test --test routing_load --features testing` runs in CI (or equivalent path)
- [x] 5.3 Run `openspec validate routing-load-verification --strict` and fix any issues

## 6. Optional follow-up (out of initial PR scope)

- [ ] 6.1 k6 `benchmarks/suite/routing-autodefault.js` + nightly job polling provider-stats
- [ ] 6.2 Stubr `Authorization` header matchers for per-credential HTTP verification in Level 2
