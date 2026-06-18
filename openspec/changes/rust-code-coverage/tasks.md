## 1. Tooling and baseline

- [ ] 1.1 Verify `mise.toml` has `llvm-tools-preview`, `cargo:cargo-llvm-cov`, `coverage:lib`, `coverage:report` (already partial — confirm `CARGO_INCREMENTAL=0`)
- [ ] 1.2 Add `coverage:gate` task: `cargo llvm-cov --workspace --all-features --lib --summary-only --fail-under-lines 48`
- [ ] 1.3 Add `lcov.info` to `.gitignore`
- [ ] 1.4 Create `docs/coverage-baseline.md` with 2026-06-18 spike numbers (workspace 49.23% lines, ai-gateway 48.76%), per-crate table, priority modules, `--lib` scope note

## 2. Fix failing multi-turn upload test (production + harness)

- [ ] 2.1 Reproduce: `cargo test -p chatgpt-web --lib uploads_oversized_context` and `cargo llvm-cov -p chatgpt-web --lib` — confirm failure mode (`mock fetch exhausted`)
- [ ] 2.2 Compare `plan_conversation_turns` turn count for 76k-word dossier vs `MockFetch` queue length and executor fetch loop in `crates/chatgpt-web/src/executor.rs`
- [ ] 2.3 Fix production if executor drops upload turns or fails to advance `parent_message_id` between turns
- [ ] 2.4 Fix test harness: size `MockFetch` from `plan.turns.len()` (dynamic callback or generated per-turn SSE meta), keep `call_count() > 7` assertion
- [ ] 2.5 Pass under plain `cargo test` and `cargo llvm-cov -p chatgpt-web --lib`
- [ ] 2.6 `mise run coverage:lib` exits 0 for full workspace — no `--ignore-run-fail`

## 3. CI coverage job (warning-only)

- [ ] 3.1 Add `coverage` job to `.github/workflows/rust-ci.yml` parallel to `test`
- [ ] 3.2 Install `cargo-llvm-cov` via `taiki-e/install-action` and `llvm-tools-preview` via rustup
- [ ] 3.3 Run `cargo llvm-cov --workspace --all-features --lib --summary-only` with `CARGO_INCREMENTAL=0`
- [ ] 3.4 Generate `lcov.info` and upload artifact `coverage-lcov`
- [ ] 3.5 Set `continue-on-error: true` on the coverage job (warning-only, no merge block)

## 4. Targeted coverage improvement (+2pp)

- [ ] 4.1 Add unit tests for `ai-gateway/src/config/validation.rs` invalid/missing field branches (target >70% file lines)
- [ ] 4.2 Add unit tests for `ai-gateway/src/config/rate_limit.rs` deserialize edge cases
- [ ] 4.3 Add unit tests for `router/budget_aware/selection.rs` — no-candidate / all-filtered path via `test_support`
- [ ] 4.4 Add unit tests for `router/budget_aware/failure.rs` or `dispatch.rs` — failure classification branch
- [ ] 4.5 Re-run `mise run coverage:lib`; update `docs/coverage-baseline.md` if workspace lines ≥ 51%

## 5. Predeploy skill update (English)

- [ ] 5.1 Add **Coverage** subsection to `.agents/skills/smart-conventional-commit-with-predeploy/SKILL.md`
- [ ] 5.2 Document: `predeploy:rust` unchanged; optional `coverage:lib`; tests required for new logic in `router/`, `config/`, `crates/*-web/`
- [ ] 5.3 Add anti-pattern rows: do not add coverage to predeploy (too slow); do not mask failing lib tests with `--ignore-run-fail`
- [ ] 5.4 Document baseline bump workflow (`coverage:lib` → update `docs/coverage-baseline.md` → optional `coverage:gate` floor raise)

## 6. Validation

- [ ] 6.1 `mise run predeploy:rust` — confirm fast path unchanged
- [ ] 6.2 `mise run coverage:lib` and `mise run coverage:gate` — both pass after improvements
- [ ] 6.3 `mise exec -- openspec validate rust-code-coverage --strict`
