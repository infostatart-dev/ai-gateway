## Context

**Current state (2026-06-18 spike):**

| Scope | Lines | Regions | Functions |
|-------|-------|---------|-----------|
| Workspace `--lib --all-features` | **49.23%** | 48.85% | 49.47% |
| `ai-gateway` crate only | **48.76%** | 48.16% | 49.79% |

**Tooling already landed (partial):** `mise.toml` has `llvm-tools-preview`, `cargo:cargo-llvm-cov`, `coverage:lib`, `coverage:report`. No CI job, no baseline file, no skill update, no gate task.

**Coverage shape:**

```
HIGH (~85-100%)          MEDIUM (~40-70%)         LOW (0%)
─────────────────        ─────────────────        ─────────────
config/credentials       router/budget_aware*       app/run.rs
config/provider_limits   middleware/mapper          endpoints/*
config/model_ladder      dispatcher/service         cli/*_login.rs
retry_after, pacing      config/validation          control_plane/ws

* budget_aware has many tests but dispatch/selection glue is thin
```

**Test scope mismatch:** `predeploy:test-lib` and coverage use `--lib`. CI `test` job runs `--tests` (integration, Redis). Integration paths (`ai-gateway/tests/*.rs`) are invisible to lib coverage — explains 0% on HTTP handlers.

**Blocker:** `chatgpt-web::uploads_oversized_context_in_multiple_turns_before_final_json` fails under llvm-cov with `mock fetch exhausted` — `MockFetch` queue too short for multi-turn upload path under instrumentation overhead.

## Goals / Non-Goals

**Goals:**

- Make coverage **visible** (local tasks + CI warning job + baseline doc).
- Fix flaky test so default `coverage:lib` exits 0.
- Raise workspace lib coverage by **+2pp** via targeted unit tests (not blanket 80% chase).
- Document contributor policy in predeploy skill (English): new code in hot modules gets tests; coverage optional locally; no predeploy slowdown.
- Prepare optional `coverage:gate` for future hard enforcement.

**Non-Goals:**

- Blocking PR merges on coverage percentage (warning-only in this change).
- Codecov SaaS integration (artifact upload is enough for now).
- `--tests` / integration coverage scope (separate future change).
- Covering `app/run.rs`, CLI login, WebSocket control plane in this tranche.
- Adding `coverage:lib` to `predeploy:rust`.

## Decisions

### D1: `cargo-llvm-cov` over tarpaulin

**Choice:** `cargo-llvm-cov` with `llvm-tools-preview`.

**Rationale:** Industry standard in 2026; LLVM region-level accuracy; used by rust-analyzer; works with workspace and `--all-features`.

**Rejected:** tarpaulin (binary instrumentation, weaker async/workspace support).

### D2: Scope `--lib` aligned with predeploy

**Choice:** CI coverage job matches `coverage:lib` (`--workspace --all-features --lib`).

**Rationale:** Fast (~2 min), no Redis, deterministic, same scope developers see locally. Integration coverage is a different metric.

**Rejected:** Full `--tests` coverage in CI (Redis service, slower, duplicates existing test job without line mapping for handlers).

### D3: CI warning-only (`continue-on-error: true`)

**Choice:** Coverage job never fails the workflow in this change.

**Rationale:** Baseline is ~49%; hard gate would block all PRs until massive test writing. Visibility first, enforcement later.

**Follow-up:** Enable `coverage:gate` in CI when baseline stabilizes above ~55% and flaky tests are gone.

### D4: Baseline in `docs/coverage-baseline.md` (not JSON)

**Choice:** Human-readable markdown with numbers and priority list.

**Rationale:** Easy to review in PRs; gate floor documented inline; no parser dependency.

**Gate implementation:** `coverage:gate` mise task uses `--fail-under-lines 48` (baseline 49.23 − ~1% buffer).

### D5: Improvement tranche — config validation + router dispatch glue

**Choice:** Focus new tests on:

1. `config/validation.rs` — invalid YAML combinations, missing required fields
2. `config/rate_limit.rs` — deserialize edge cases (raises low 15% module)
3. `router/budget_aware/selection.rs` / `dispatch.rs` / `failure.rs` — no-candidate, filtered-out, failure-class paths using `test_support`

**Rejected:** Endpoint harness tests (belong in integration layer, wrong tool for lib %).

### D6: Failing lib test = bug in product or harness (fix both layers)

**Choice:** Treat `uploads_oversized_context_in_multiple_turns_before_final_json` as a **regression guard** for multi-turn context upload, not a flaky mock nuisance.

**Investigation order:**

1. Log/compare `plan.turns.len()` for the 76k-word dossier vs `MockFetch` queue length.
2. If executor requests more fetches than the mock provides → fix mock to emit per-turn SSE meta (`conversation_id`, `assistant_message_id`) for every upload + final turn.
3. If executor stops early or errors despite sufficient mocks → fix `Executor::execute` loop (production bug).
4. Verify under both `cargo test` and `cargo llvm-cov` (instrumentation must not change turn count).

**Forbidden:** `--ignore-run-fail`, weakening `call_count() > 7`, or deleting the test.

**Rationale:** Coverage instrumentation exposed a real gap — either under-provisioned mocks or an executor that does not survive full upload plans. Masking failures would ship broken large-context structured output to users.

### D7: Predeploy skill — optional measure, required tests for new logic

**Choice:** Skill text adds a **Coverage** subsection:

| Situation | Action |
|-----------|--------|
| Every rust commit | `predeploy:rust` only |
| Touch `router/`, `config/`, `crates/*-web/` | Add/extend `#[test]` in same PR |
| Before baseline bump PR | `mise run coverage:lib` + update `docs/coverage-baseline.md` |
| Optional deep check | `mise run coverage:gate` |

**Anti-patterns table:** add row "Run coverage on every commit" → "Too slow; CI warning suffices".

### D8: CI job structure

**Choice:** New parallel job in `rust-ci.yml`:

```yaml
coverage:
  runs-on: ubuntu-latest
  continue-on-error: true
  steps:
    - checkout
    - rust-cache
    - taiki-e/install-action: cargo-llvm-cov
    - rustup component add llvm-tools-preview
    - cargo llvm-cov --workspace --all-features --lib --summary-only
    - cargo llvm-cov --workspace --all-features --lib --lcov --output-path lcov.info
    - upload-artifact: coverage-lcov
```

Runs on same `paths` filter as existing jobs. `CARGO_INCREMENTAL=0` in env for stable llvm-cov builds.

## Risks / Trade-offs

| Risk | Mitigation |
|------|------------|
| Coverage job adds ~2–3 min CI time | Parallel job; cache via Swatinem/rust-cache |
| Lib % misleads (HTTP paths at 0%) | Document in baseline: integration tests cover handlers |
| Contributors ignore warning job | Skill + baseline priorities; future hard gate |
| Upload regression recurs | Keep multi-turn test; mock sized from `plan_conversation_turns`; no `--ignore-run-fail` |
| `lcov.info` committed by mistake | Add to `.gitignore` |

## Migration Plan

1. Land mise tasks + baseline doc (partially done).
2. Fix chatgpt-web flaky test.
3. Add CI coverage job (warning-only).
4. Add targeted unit tests (+2pp).
5. Update predeploy skill.
6. Add `coverage:gate` task + `.gitignore` for `lcov.info`.
7. Validate with `mise run coverage:lib` and `openspec validate --strict`.

**Rollback:** Remove CI job and gate task; keep mise tasks harmless.

## Open Questions

- **When to enable hard CI gate?** Suggest after baseline ≥ 55% and two green weeks of warning job.
- **Codecov later?** Optional if team wants PR diff coverage UI; not required for this change.
- **Per-crate gates?** `ai-gateway` vs `chatgpt-web` floors differ; defer until per-crate baselines documented.
