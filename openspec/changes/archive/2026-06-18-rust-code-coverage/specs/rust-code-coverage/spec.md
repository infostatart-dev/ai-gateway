# rust-code-coverage

## Purpose

Measurable, incremental Rust lib test coverage for the ai-gateway workspace: local
instrumentation via `cargo-llvm-cov`, a checked-in baseline, CI visibility (warning-only),
contributor policy in the predeploy skill, and targeted tests for high-value gaps — without
blocking the fast predeploy path or requiring live provider keys.

## ADDED Requirements

### Requirement: Canonical coverage tooling via mise

The workspace SHALL measure Rust lib test coverage using `cargo-llvm-cov` with LLVM
source-based instrumentation (`llvm-tools-preview`). The tool SHALL be installed through
`mise.toml` (not ad-hoc `cargo install` in contributor shells). Local tasks SHALL exist:

- `coverage:lib` — workspace summary (`--all-features --lib --summary-only`)
- `coverage:report` — LCOV output at repo root (`lcov.info`)
- `coverage:gate` — optional floor check (`--fail-under-lines` at checked-in baseline minus buffer)

`coverage:lib` and `coverage:report` SHALL NOT be dependencies of `predeploy:rust`.

#### Scenario: Contributor runs local coverage summary

- **WHEN** a contributor runs `mise run coverage:lib`
- **THEN** `cargo-llvm-cov` executes workspace lib tests with instrumentation
- **AND** prints line/region/function totals to stdout

#### Scenario: Coverage report for IDE review

- **WHEN** a contributor runs `mise run coverage:report`
- **THEN** an `lcov.info` file is written at the repository root
- **AND** the file is listed in `.gitignore` (not committed)

### Requirement: Checked-in coverage baseline

The repository SHALL maintain `docs/coverage-baseline.md` documenting:

- Measurement date and command (`mise run coverage:lib`)
- Workspace totals (lines, regions, functions)
- Per-crate line coverage for workspace members with lib tests
- Priority improvement areas (modules under 50% that contain routing/config logic)
- Explicit note that `--lib` excludes `ai-gateway/tests/` integration coverage

The baseline SHALL be updated when a deliberate coverage improvement lands (not on every commit).

#### Scenario: Baseline documents current floor

- **WHEN** a reader opens `docs/coverage-baseline.md`
- **THEN** they see the workspace line-coverage percentage and per-crate breakdown
- **AND** see which modules are prioritized for the next improvement tranche

#### Scenario: Gate uses baseline minus buffer

- **WHEN** `mise run coverage:gate` executes
- **THEN** it fails if workspace line coverage drops below the documented floor (baseline − 1%)
- **AND** prints the current vs expected totals

### Requirement: CI coverage job (warning-only)

Rust CI SHALL include a `coverage` job that runs on the same path triggers as existing Rust
jobs. The job SHALL:

1. Install `llvm-tools-preview` and `cargo-llvm-cov` (via `taiki-e/install-action`)
2. Run `cargo llvm-cov --workspace --all-features --lib --summary-only`
3. Generate `lcov.info` and upload it as a workflow artifact
4. Use `continue-on-error: true` so PR merges are not blocked by coverage regressions yet
5. Print the `TOTAL` summary line in the job log for visibility

The job SHALL NOT require Redis or live API keys (lib tests only).

#### Scenario: PR shows coverage summary without blocking merge

- **WHEN** a pull request triggers Rust CI
- **THEN** the `coverage` job runs in parallel with `test`
- **AND** the job log contains workspace line-coverage totals
- **AND** a failed coverage step does not fail the overall workflow while warning-only mode is active

#### Scenario: LCOV artifact available for download

- **WHEN** the coverage job completes successfully
- **THEN** `lcov.info` is attached as a GitHub Actions artifact named `coverage-lcov`
- **AND** contributors can download it for local diff review

### Requirement: Zero failing lib tests (hard invariant)

The workspace SHALL have zero failing lib tests before coverage baseline or CI coverage
artifacts are considered valid. Any failing workspace lib test is a **defect** — not an
acceptable coverage artifact. Coverage workflows SHALL NOT suppress, skip, or ignore failing
tests (`--ignore-run-fail`, `--no-fail-fast`, or equivalent flags are forbidden in
`mise.toml` tasks and CI coverage steps).

A lib test that fails under `cargo llvm-cov` but passes under plain `cargo test` still counts
as a failure: the implementation or test harness MUST be corrected before this change is
considered complete.

#### Scenario: Coverage run fails when any lib test fails

- **WHEN** any workspace lib test panics or returns `Err` during `mise run coverage:lib`
- **THEN** the command exits non-zero
- **AND** no coverage percentage is treated as the new baseline until all lib tests pass

#### Scenario: No failure suppression in tooling

- **WHEN** a contributor inspects `mise.toml` coverage tasks and the CI coverage job
- **THEN** neither uses `--ignore-run-fail` nor documents failure suppression as policy
- **AND** the predeploy skill lists masking test failures as an anti-pattern

### Requirement: Multi-turn context upload correctness (chatgpt-web)

The `chatgpt-web` executor SHALL upload oversized user context across **multiple conversation
turns** (context-upload parts via `plan_conversation_turns` / `plan_web_chunks`) and only
then issue the final structured-output request. The regression test
`uploads_oversized_context_in_multiple_turns_before_final_json` encodes this contract.

The currently failing run (`mock fetch exhausted` under `cargo llvm-cov`) MUST be resolved by
**root-cause fix**, in this order of investigation:

1. **Production path:** `Executor::execute` multi-turn loop — ensure every planned upload turn
   receives a fetch response and advances `conversation_id` / `parent_message_id` correctly.
2. **Test harness:** `MockFetch` queue — MUST supply one SSE response per fetch call for warmup
   + every upload turn + final turn (derive count from `plan_conversation_turns` for the test
   dossier, not a hard-coded short queue).
3. **Assertions:** `call_count() > 7` remains — proving multi-turn upload happened, not a
   single-shot shortcut.

Fixing only the mock without verifying production turn advancement is insufficient if the
executor drops turns or mis-counts fetches.

#### Scenario: Oversized dossier completes all upload turns

- **WHEN** `Executor::execute` receives a `json_schema_required` body whose user message exceeds
  one upload chunk (`~76k-word dossier` in the regression test)
- **THEN** the executor performs more than seven upstream fetches (warmup + uploads + final)
- **AND** returns HTTP 200 with valid JSON on success
- **AND** does not return `mock fetch exhausted` or `chunk plan produced no final turn`

#### Scenario: Test passes under plain and instrumented runs

- **WHEN** `cargo test -p chatgpt-web --lib uploads_oversized_context_in_multiple_turns_before_final_json` runs
- **THEN** the test passes
- **AND** the same test passes under `cargo llvm-cov -p chatgpt-web --lib --summary-only`
- **AND** `mise run coverage:lib` exits zero for the full workspace

### Requirement: Targeted coverage improvement tranche

The change SHALL add focused lib unit tests that raise coverage in high-value, under-tested
modules without chasing 0% bootstrap/CLI/HTTP glue in this tranche. Priority targets:

| Module area | Rationale |
|-------------|-----------|
| `ai-gateway/src/config/validation.rs` | Config errors surface at startup; currently ~55% lines |
| `ai-gateway/src/config/rate_limit.rs` | Rate-limit config parsing; currently ~15% lines |
| `ai-gateway/src/router/budget_aware/dispatch.rs` | Core autodefault dispatch path |
| `ai-gateway/src/router/budget_aware/selection.rs` | Candidate selection under constraints |
| `ai-gateway/src/router/budget_aware/failure.rs` | Failure classification for failover |

New tests SHALL follow existing patterns (`test_support`, injected mocks, no live keys).
The tranche SHALL raise workspace lib line coverage by at least **+2 percentage points**
from the checked-in baseline.

#### Scenario: Validation module gains edge-case tests

- **WHEN** validation tests run under coverage
- **THEN** invalid config combinations in `config/validation.rs` branches are exercised
- **AND** line coverage for that file exceeds 70%

#### Scenario: Dispatch selection path has unit coverage

- **WHEN** budget-aware dispatch unit tests run
- **THEN** at least one test covers a no-candidate failure path in `selection.rs` or `dispatch.rs`
- **AND** the test uses synthetic credentials from test fixtures only

### Requirement: Predeploy skill coverage policy (English)

The smart-conventional-commit predeploy skill SHALL document coverage workflow:

- **Default:** `predeploy:rust` unchanged (fmt → clippy → test-lib); coverage not required per commit
- **New Rust code** in `router/`, `config/`, `crates/*-web/`: author SHOULD add or extend lib unit tests in the same PR
- **Optional local check:** run `mise run coverage:lib` when touching covered modules; run `coverage:gate` before proposing a baseline bump
- **CI:** coverage job is informational until a future change enables hard gates
- **Anti-pattern:** do not add `coverage:lib` to `predeploy:rust` (too slow for agent/human flow)
- **Anti-pattern:** do not use `--ignore-run-fail` or accept failing lib tests to obtain a coverage number — fix the test and the code it guards first

#### Scenario: Agent classifies rust diff with coverage hint

- **WHEN** an agent commits changes under `ai-gateway/src/router/`
- **THEN** the predeploy skill instructs adding unit tests for new branches
- **AND** does not require `mise run coverage:lib` on every commit

#### Scenario: Baseline bump workflow documented

- **WHEN** a contributor lands a coverage improvement tranche
- **THEN** they update `docs/coverage-baseline.md` and optionally raise `coverage:gate` floor
- **AND** mention the new totals in the commit body
