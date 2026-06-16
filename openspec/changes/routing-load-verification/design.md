## Context

The gateway already validates autodefault routing through:

- **Unit tests:** `credential_balance.rs` (round-robin), `budget_aware/tests.rs` (rank order), `payload.rs` (filter logic).
- **Sequential router integration:** `credential_failover.rs`, `failover_integration.rs` via `push_test_call_response` (global FIFO queue).
- **End-to-end Harness tests:** `provider_observability.rs` (stats API), rate-limit tests (sequential).
- **Throughput benchmarks:** `benchmarks/` k6 suites against mock LLM server (~3k RPS, small body) — no autodefault credential mix.

**Gap:** no concurrent verification that, under burst load with large `json_schema` payloads, the router:

1. Fairly rotates across free Gemini credential slots.
2. Respects payload-aware pre-filtering (no doomed hops to TPM-limited providers).
3. Fails over correctly (RPM sibling vs daily-quota skip vs paid Gemini).
4. Keeps ChatGPT Web as last resort — not hit while free APIs succeed.
5. Respects pacing and decision-shaper backpressure without corrupting shared router state.

**Constraints:** CI must not call live providers, spend tokens, or require production secrets. Tests must be deterministic and serializable where global state is shared (`CredentialRoundRobin`, `ProviderState`, pacing gates).

**Existing observability hook:** `GET /v1/observability/provider-stats` returns per-`(provider, credential)` attempt totals since process start — ideal assertion surface (see `provider-observability` spec).

## Goals / Non-Goals

**Goals:**

- One unified **routing load verification** framework reusable as autodefault evolves.
- **Level 1 (PR):** concurrent router tests with synthetic credentials and per-credential mock responses.
- **Level 2 (PR):** Harness + Stubr end-to-end tests for round-robin and payload filter under concurrent HTTP dispatch.
- Shared **fixture** (`RoutingLoadProfile`) mirroring prod-like autodefault: four free Gemini slots, optional `gemini-default`, ChatGPT Web last resort, fat `json_schema` body.
- **Stats-based assertions** with tolerance bands (not exact counts) for fairness checks.
- Per-credential test hook replacing global FIFO for concurrent failover scenarios.
- Contributor doc: how to add a scenario in one file.

**Non-Goals:**

- Changing production routing behavior (test-only infrastructure).
- Live provider load tests or token spend.
- Full ChatGPT Web browser-session HTTP mock in Level 2 (last-resort scenarios use Level 1 injection until a dedicated stub exists).
- Replacing existing k6 throughput benchmarks (`benchmarks/suite/test.js`).
- Expanding embedded credentials catalog beyond four free Gemini (parameterized `N` in tests; catalog expansion is a separate change).
- Level 3 k6 routing suite in initial implementation (optional follow-up).

## Decisions

### D1: Assertion surface — provider-stats, not stub call counts

**Choice:** Primary assertions read `GET /v1/observability/provider-stats` rows keyed by `(provider, credential)`.

**Rationale:** Gateway already records every upstream attempt with credential id. Stubr stubs match URL/method only today — not per-credential routing. Stats are the single source of truth aligned with production observability.

**Alternative rejected:** Count Stubr invocations per stub name — cannot distinguish `gemini-free` vs `gemini-free-2` without new request matchers on every stub.

### D2: Per-credential mock map for concurrent tests

**Choice:** Extend `call.rs` test hooks with a map `{ credential_id → response queue or fn }`, falling back to existing global FIFO for backward compatibility.

**Rationale:** Current `push_test_call_response` global queue races under `join!` / concurrent dispatch — breaks failover scenario tests.

**Alternative rejected:** Sequential-only load tests (32 requests in a loop) — weaker signal for race bugs in round-robin and pacing.

### D3: Two-level pyramid in one framework, not split changes

**Choice:** Single capability `routing-load-verification` with Level 1 (router-only) and Level 2 (Harness) sharing `fixture.rs` + `assert_stats.rs`.

**Rationale:** User requested one unified evolution path, not fragmented test suites.

**Level mapping:**

| Level | Location | Runs | Validates |
|-------|----------|------|-----------|
| L1 | `tests/routing_load/` router scenarios | every PR | RR, failover, last-resort, pacing (paused time) |
| L2 | `tests/routing_load/` harness scenarios | every PR | HTTP path + stats + payload filter |
| L3 | `benchmarks/suite/routing-autodefault.js` | nightly (future) | sustained burst + stats poll |

### D4: Fixture profile — `AutodefaultProdLike`

**Choice:** Test secrets YAML builder registering:

- `gemini-free`, `gemini-free-2`, `gemini-free-3`, `gemini-free-4` with distinct synthetic keys `free-{n}-key`.
- Optional `gemini-default` with `paid-key`.
- ChatGPT Web via injected `BudgetCandidate` in L1 (not live session file in CI).

Router config: `budget-aware-capability-after`, decision enabled, `tier-cascade: free-up` (matches autodefault policy).

Payload: reusable fat `json_schema` body (derived from `failover_integration.rs` pattern) with deterministic filler to target ~N estimated input tokens.

Parameter `N_FREE_GEMINI: 1..4` (extendable to 8 when catalog grows).

### D5: Fairness tolerance — band, not exact equality

**Choice:** For `M` client requests and `K` active free Gemini slots, each slot's terminal attempts must fall in `[M/K × (1−ε), M/K × (1+ε)]` with default `ε = 0.25`, and chi-square optional for larger runs.

**Rationale:** Round-robin rotates **first** slot per request; concurrent scheduling introduces benign skew. Exact equality is flaky.

### D6: Three bottleneck layers — separate scenario files

**Choice:** Do not combine shaper, router, and pacing into one mega-test.

| Scenario file | Layer under test |
|---------------|------------------|
| `round_robin.rs` | CredentialRoundRobin under concurrent success |
| `payload_filter.rs` | payload-aware pre-filter with fat json_schema |
| `failover_rpm.rs` | transient 429 → sibling |
| `failover_quota.rs` | daily quota → skip siblings → paid |
| `chatgpt_last_resort.rs` | ChatGPT Web only when free chain exhausted |
| `pacing_burst.rs` | PacingGate serializes chatgpt-web (paused clock) |
| `shaper_backpressure.rs` | Decision shaper limits concurrent free-tier slots |

**Rationale:** Failure isolation — one broken layer does not obscure others.

### D7: ChatGPT Web last-resort state machine (test invariant)

**Formal invariant for scenarios:**

- While any free API credential succeeds for a request class, `chatgpt-web-default.attempts == 0`.
- When fixture marks all free Gemini slots in cooldown or filtered out, terminal credential may be `chatgpt-web-default`.
- ChatGPT Web attempts never exceed client requests that explicitly require paid-browser capability.

Document in spec as normative scenarios.

### D8: Serial execution and isolated AppState

**Choice:** All routing load tests use `#[serial_test::serial]` and fresh `AppState` / Harness per test case.

**Rationale:** Shared `CredentialRoundRobin`, `ProviderState`, and pacing registry state leak between tests otherwise.

## Risks / Trade-offs

| Risk | Mitigation |
|------|------------|
| Per-credential mock hook adds test-only complexity | Keep global FIFO as fallback; document both APIs |
| L2 cannot stub ChatGPT Web HTTP | L1 injection for last-resort; document gap |
| Stats snapshot is process-global | Fresh Harness per test; assert deltas or reset via new process |
| Fairness flakes on CI timing | Tolerance bands; moderate concurrency (32, not 1000) in PR tier |
| Decision shaper ceiling masks router RR | Separate shaper scenario with known limits |
| Stubr latency bug (`load_balance.rs` ignored) | Do not use stubr global_delay for pacing tests; use paused tokio time in L1 |
| Expanding to 8 Gemini keys | Parameterized fixture; no catalog change in this change |

## Migration Plan

1. Land per-credential test hook (backward compatible).
2. Add shared fixture + assert helpers.
3. Add L1 scenarios incrementally (RR first, then failover, then last-resort).
4. Add L2 harness scenario(s).
5. Document contributor workflow.
6. (Optional) k6 nightly job referencing same profile JSON.

No production migration — tests only. Rollback = revert test files and hook extension.

## Open Questions

1. **Level 2 Stubr credential matching:** add `Authorization` header matchers to gemini stubs, or rely solely on stats (stats-only is sufficient for v1).
2. **Default concurrency for PR tier:** 32 concurrent vs 16 (align with decision shaper `free-tier: 16` in decision-example).
3. **Release version tag** for this capability in spec (follow sibling specs' `0.3.0-beta.N` pattern when implementation ships).
