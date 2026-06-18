## Why

The workspace has strong unit and integration test suites but **no measured line coverage baseline**, no CI visibility, and no contributor guidance for improving coverage incrementally. A recent `cargo-llvm-cov` spike shows **~49% lib line coverage** (workspace, `--all-features --lib`) while large production paths (`app/run`, HTTP endpoints, CLI login flows) sit at 0% because they are exercised only via integration tests outside the `--lib` scope. Without instrumentation and policy, new routing/config code can ship without tests and regressions go unnoticed until runtime.

## What Changes

- Establish **`cargo-llvm-cov`** as the canonical coverage tool, installed via `mise.toml` (`llvm-tools-preview` + `cargo:cargo-llvm-cov`) with local tasks `coverage:lib` and `coverage:report`.
- Record a **checked-in baseline** (`docs/coverage-baseline.md`) with workspace and per-crate line/region totals and improvement priorities.
- Add a **CI coverage job** (warning-only, `continue-on-error: true`) that runs on every Rust CI trigger, prints summary, uploads `lcov.info` artifact — no merge gate yet.
- **Fix the failing `chatgpt-web` lib test** (`uploads_oversized_context_in_multiple_turns_before_final_json`) and the underlying **multi-turn context upload** behavior it guards — oversized dossiers MUST complete all upload turns before the final JSON response. No `--ignore-run-fail`, no lowered assertions; root-cause fix in production code and/or test harness.
- Add a **targeted lib test tranche** for high-value, low-coverage modules (router dispatch glue, validation edges) — not a blind push to 80%.
- Update **predeploy skill** (English) with scope-aware coverage guidance: measure optionally, improve new code in covered modules, do not block commits on coverage yet.
- Add optional `coverage:gate` mise task with `--fail-under-lines` at baseline minus buffer — **local/CI opt-in only**, not in `predeploy:rust`.

## Capabilities

### New Capabilities

- `rust-code-coverage`: Instrumentation, baseline tracking, CI warning job, contributor policy, and incremental improvement targets for Rust lib test coverage across the workspace.

### Modified Capabilities

<!-- No production routing or API requirement changes — developer workflow and test infrastructure only. -->

## Impact

- **Tooling:** `mise.toml` (already partially updated), new `docs/coverage-baseline.md`, `.gitignore` entry for `lcov.info` if not present.
- **CI:** `.github/workflows/rust-ci.yml` — new `coverage` job parallel to `test`.
- **Skills:** `.agents/skills/smart-conventional-commit-with-predeploy/SKILL.md` — coverage section.
- **Production + tests:** `crates/chatgpt-web` executor multi-turn upload path (if under-delivering fetch rounds) and `executor/tests.rs` mock alignment; add focused unit tests in `ai-gateway` router/validation hot paths.
- **Not affected:** production routing behavior, release artifacts, predeploy fast path (fmt → clippy → test-lib stays unchanged).
